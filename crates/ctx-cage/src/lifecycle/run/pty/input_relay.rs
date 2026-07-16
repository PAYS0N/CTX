//! Host stdin → PTY master relay for [`super::run_on_pty`].
//!
//! Cancellable via a self-pipe rather than left to block in `read`
//! forever: a detached, unjoined relay outlives the cage session and
//! races any later read of the real host stdin — e.g. `ctx-run`'s
//! post-session summarize y/n prompt, which then hangs because the
//! leaked thread steals the typed answer instead of the prompt's own
//! `read_line`.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::thread;

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

use super::dup_fd;

/// Host stdin → PTY master, until [`stop_input_relay`] cancels it.
pub(super) fn spawn_input_relay(
    to_master: File,
    cancel: OwnedFd,
) -> Option<thread::JoinHandle<()>> {
    let stdin = dup_fd(std::io::stdin()).ok()?;
    Some(thread::spawn(move || {
        input_relay_loop(stdin, to_master, &cancel);
    }))
}

/// Wake [`spawn_input_relay`]'s poll loop (dropping the write end raises
/// `POLLHUP` on the cancel fd) and join it, so no thread is left
/// holding a duped stdin fd once this returns.
pub(super) fn stop_input_relay(cancel_write: OwnedFd, input: Option<thread::JoinHandle<()>>) {
    drop(cancel_write);
    if let Some(handle) = input {
        let _ = handle.join();
    }
}

/// Poll `stdin`/`cancel` rather than blocking directly in `read`, so the
/// loop can be woken by [`stop_input_relay`] instead of only by stdin
/// EOF (which never happens while the host terminal stays open).
fn input_relay_loop(mut stdin: File, mut to_master: File, cancel: &OwnedFd) {
    let mut buf = [0_u8; 8192];
    loop {
        let fds = [
            PollFd::new(stdin.as_fd(), PollFlags::POLLIN),
            PollFd::new(cancel.as_fd(), PollFlags::POLLIN),
        ];
        if !wait_for_input(fds) {
            break;
        }
        match stdin.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let Some(chunk) = buf.get(..n) else { break };
                if to_master.write_all(chunk).is_err() || to_master.flush().is_err() {
                    break;
                }
            },
        }
    }
}

/// Block until stdin is readable; returns `false` on a poll error or
/// once `cancel` fires (data or the writer dropped), meaning the caller
/// should stop rather than read.
fn wait_for_input(mut fds: [PollFd; 2]) -> bool {
    if poll(&mut fds, PollTimeout::NONE).is_err() {
        return false;
    }
    if fds[1].any().unwrap_or(true) {
        return false;
    }
    fds[0].any().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use nix::pty::openpty;
    use nix::unistd::pipe;

    use super::*;

    /// Regression test for the ctx-run y/n hang: cancelling must wake a
    /// blocked `input_relay_loop` promptly rather than leaving it to
    /// read forever and race a later read of the real stdin.
    #[test]
    fn cancel_wakes_a_blocked_relay_loop() {
        let pair = openpty(None, None).expect("openpty");
        let stdin_stand_in = File::from(pair.slave);
        let to_master = File::from(pair.master);
        let (cancel_read, cancel_write) = pipe().expect("pipe");

        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            input_relay_loop(stdin_stand_in, to_master, &cancel_read);
            let _ = done_tx.send(());
        });

        drop(cancel_write);
        done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("input_relay_loop should exit once cancelled, not hang");
    }
}
