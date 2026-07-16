//! Interactive PTY relay.
//!
//! `--unshare-pid` puts the caged process in a private PID namespace;
//! if it inherits the host's controlling terminal directly,
//! `tcgetpgrp`/`tcsetpgrp` can't resolve across the boundary and
//! Node's readline busy-retries (ADR-034 backlog). Fix: allocate a
//! private pseudoterminal on the host, hand its slave to the cage as
//! stdio (the in-cage `setsid --ctty` in [`super::claude_cmd`] makes
//! it the child's controlling terminal inside the new
//! session/namespace), and relay bytes to/from the host's real
//! terminal. Full isolation is preserved.
//!
//! Only the *safe* wrappers of `nix`/`rustix` are used here; the
//! workspace's `unsafe_code = "forbid"` still holds.

use std::ffi::OsString;
use std::fs::File;
use std::io::IsTerminal;
use std::os::fd::{AsFd, OwnedFd};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;

use nix::pty::{openpty, OpenptyResult, Winsize};
use nix::sys::signal::{SigSet, Signal};
use nix::sys::signalfd::SignalFd;
use nix::sys::termios::{self, SetArg, Termios};

use crate::error::CageError;
use crate::proxy::pump;

/// Restores the host terminal's original line settings on drop, so raw
/// mode never leaks past the session — covers early return and panic.
struct RawGuard(Termios);

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, &self.0);
    }
}

/// Run `argv` (a full `bwrap` argv, program at index 0) on a private
/// PTY, relaying bytes to/from the host terminal.
///
/// Returns `Ok(None)` when stdin is not a real terminal (piped / CI /
/// tests) so the caller falls back to plain stdio inheritance.
///
/// # Errors
///
/// [`CageError::Io`] on PTY, termios, or spawn failures.
pub(super) fn run_on_pty(argv: &[OsString]) -> Result<Option<ExitStatus>, CageError> {
    if !std::io::stdin().is_terminal() {
        return Ok(None);
    }
    // Block SIGWINCH before spawning any thread so all relay threads
    // inherit the block and the signalfd is the sole reader.
    let sigset = block_sigwinch()?;
    let orig = termios::tcgetattr(std::io::stdin()).map_err(io_err)?;
    let guard = enter_raw(&orig)?;
    let pair = open_pty(&orig)?;
    let child = spawn_on_slave(argv, pair.slave)?;
    let status = relay_until_exit(pair.master, sigset, child)?;
    drop(guard);
    Ok(Some(status))
}

/// Block `SIGWINCH` in the calling thread (inherited by threads spawned
/// after this) so it is delivered only via the signalfd.
fn block_sigwinch() -> Result<SigSet, CageError> {
    let mut set = SigSet::empty();
    set.add(Signal::SIGWINCH);
    set.thread_block().map_err(io_err)?;
    Ok(set)
}

/// Put the host terminal into raw mode; the returned guard restores the
/// original settings on drop.
fn enter_raw(orig: &Termios) -> Result<RawGuard, CageError> {
    let mut raw = orig.clone();
    termios::cfmakeraw(&mut raw);
    termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, &raw).map_err(io_err)?;
    Ok(RawGuard(orig.clone()))
}

/// Allocate the PTY pair (slave gets the host's *cooked* termios and
/// current size). `openpty(3)` doesn't set `CLOEXEC`, and `bwrap`
/// forwards arbitrary inherited fds into the sandbox rather than
/// closing them, so [`set_cloexec`] closes that leak; `dup2`
/// (`Stdio::from` in [`spawn_on_slave`]) clears it again on the
/// target, so the deliberate slave→stdio wiring is unaffected.
fn open_pty(term: &Termios) -> Result<OpenptyResult, CageError> {
    let pair = openpty(host_winsize().as_ref(), Some(term)).map_err(io_err)?;
    set_cloexec(&pair.master)?;
    set_cloexec(&pair.slave)?;
    Ok(pair)
}

/// Mark `fd` close-on-exec.
fn set_cloexec<Fd: AsFd>(fd: &Fd) -> Result<(), CageError> {
    rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC).map_err(|e| CageError::Io(e.into()))
}

/// Spawn the caged process with the PTY slave as stdin/stdout/stderr.
fn spawn_on_slave(argv: &[OsString], slave: OwnedFd) -> Result<Child, CageError> {
    let (prog, rest) = argv
        .split_first()
        .ok_or_else(|| CageError::Io(std::io::Error::other("empty bwrap argv")))?;
    let stdin = slave.try_clone()?;
    let stdout = slave.try_clone()?;
    Ok(Command::new(prog)
        .args(rest)
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(slave))
        .spawn()?)
}

/// Wire up the relay threads, wait for the cage to exit, and return its
/// status. The parent holds no slave fd, so master read sees EOF when
/// the child closes its stdio.
fn relay_until_exit(
    master: OwnedFd,
    sigset: SigSet,
    mut child: Child,
) -> Result<ExitStatus, CageError> {
    let master = File::from(master);
    let to_child = master.try_clone()?;
    let for_winch = master.try_clone()?;
    spawn_input_relay(to_child);
    spawn_winch_relay(sigset, for_winch)?;
    let output = spawn_output_relay(master);
    let status = child.wait()?;
    let _ = output.join();
    Ok(status)
}

/// Detached: host stdin → PTY master. Blocks in `read`; dies with the
/// process (it may linger a moment after the cage exits — harmless).
fn spawn_input_relay(to_master: File) {
    if let Ok(stdin) = dup_fd(std::io::stdin()) {
        thread::spawn(move || pump(stdin, to_master));
    }
}

/// Joined: PTY master → host stdout. Returns when the master hits EOF at
/// child exit.
fn spawn_output_relay(from_master: File) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        if let Ok(stdout) = dup_fd(std::io::stdout()) {
            pump(from_master, stdout);
        }
    })
}

/// Detached: forward host resize events onto the PTY master, so the cage
/// sees window-size changes live.
fn spawn_winch_relay(set: SigSet, master: File) -> Result<(), CageError> {
    let sfd = SignalFd::new(&set).map_err(io_err)?;
    thread::spawn(move || winch_loop(&sfd, &master));
    Ok(())
}

/// Block on `SIGWINCH`; on each one copy the host terminal's size onto
/// the PTY master (which then `SIGWINCH`es the caged session).
fn winch_loop(sfd: &SignalFd, master: &File) {
    while let Ok(Some(_)) = sfd.read_signal() {
        if let Ok(ws) = rustix::termios::tcgetwinsize(std::io::stdin()) {
            let _ = rustix::termios::tcsetwinsize(master, ws);
        }
    }
}

/// Current host-terminal size as a `nix` [`Winsize`], or `None` if it
/// can't be read (the PTY then opens at the kernel default).
fn host_winsize() -> Option<Winsize> {
    rustix::termios::tcgetwinsize(std::io::stdin())
        .ok()
        .map(to_nix_winsize)
}

/// Convert a `rustix` winsize to the `libc`/`nix` layout `openpty` wants.
const fn to_nix_winsize(ws: rustix::termios::Winsize) -> Winsize {
    Winsize {
        ws_row: ws.ws_row,
        ws_col: ws.ws_col,
        ws_xpixel: ws.ws_xpixel,
        ws_ypixel: ws.ws_ypixel,
    }
}

/// `dup` a borrowed fd into an owned [`File`] (so relay threads can own
/// a handle without closing the caller's stdin/stdout).
fn dup_fd<Fd: AsFd>(fd: Fd) -> Result<File, CageError> {
    Ok(File::from(fd.as_fd().try_clone_to_owned()?))
}

/// Map a `nix` errno into [`CageError::Io`] (both wrap an OS error).
fn io_err(e: nix::Error) -> CageError {
    CageError::Io(e.into())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;

    /// The winsize conversion preserves every field.
    #[test]
    fn winsize_conversion_is_field_for_field() {
        let src = rustix::termios::Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 640,
            ws_ypixel: 480,
        };
        let out = to_nix_winsize(src);
        assert_eq!(out.ws_row, 24);
        assert_eq!(out.ws_col, 80);
        assert_eq!(out.ws_xpixel, 640);
        assert_eq!(out.ws_ypixel, 480);
    }

    /// A raw PTY pair passes bytes master→slave verbatim, proving the
    /// `openpty` plumbing the relay depends on. Uses a fresh pty, never
    /// the host terminal, so it is safe under `ctx-verify`.
    #[test]
    fn openpty_pair_relays_bytes() {
        let mut raw =
            termios::tcgetattr(std::io::stdin()).unwrap_or_else(|_| default_raw_termios());
        termios::cfmakeraw(&mut raw);
        let pair = openpty(None, Some(&raw)).expect("openpty");
        let mut master = File::from(pair.master);
        let mut slave = File::from(pair.slave);
        master.write_all(b"hi").expect("write master");
        master.flush().expect("flush");
        let mut buf = [0_u8; 2];
        slave.read_exact(&mut buf).expect("read slave");
        assert_eq!(&buf, b"hi");
    }

    /// Fallback termios when the test harness has no tty on stdin: an
    /// empty pty's own settings, made raw by the caller.
    fn default_raw_termios() -> Termios {
        let pair = openpty(None, None).expect("openpty for default termios");
        termios::tcgetattr(pair.slave).expect("tcgetattr slave")
    }
}
