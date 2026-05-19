#!/usr/bin/env python3
"""Host PTY pump for the cage's interactive mode.

Runs `argv` on a FRESH pty whose master this process owns, relaying to
the real terminal. The cage (a child of `argv`, e.g. bwrap) sees only
its own pty slave as a controlling terminal — so a TIOCSTI/TIOCSTI-style
injection from inside lands in *this* pty (which we read), never the
user's real terminal. That keeps `--new-session` + interactive sound
even on hosts where dev.tty.legacy_tiocsti is not 0.

Usage: pty-relay.py <cmd> [args...]   (exit status = child's)
"""

from __future__ import annotations

import array
import fcntl
import os
import select
import signal
import sys
import termios
import tty


def _copy_winsize(dst_fd: int) -> None:
    """Mirror the real stdout's window size onto the pty master `dst_fd`."""
    try:
        sz = fcntl.ioctl(sys.stdout.fileno(), termios.TIOCGWINSZ, b"\0" * 8)
        fcntl.ioctl(dst_fd, termios.TIOCSWINSZ, sz)
    except OSError:
        pass


def main(argv: list[str]) -> int:
    if not argv:
        print("pty-relay: no command", file=sys.stderr)
        return 2

    master_fd, slave_fd = os.openpty()
    pid = os.fork()
    if pid == 0:  # child: become session leader, adopt slave as ctty
        os.setsid()
        try:
            fcntl.ioctl(slave_fd, termios.TIOCSCTTY, 0)
        except OSError:
            pass
        for fd in (0, 1, 2):
            os.dup2(slave_fd, fd)
        if slave_fd > 2:
            os.close(slave_fd)
        os.close(master_fd)
        os.execvp(argv[0], argv)
        os._exit(127)  # unreachable on success

    os.close(slave_fd)
    stdin_fd = sys.stdin.fileno()
    is_tty = os.isatty(stdin_fd)
    saved = termios.tcgetattr(stdin_fd) if is_tty else None
    if is_tty:
        tty.setraw(stdin_fd)
    _copy_winsize(master_fd)
    signal.signal(signal.SIGWINCH, lambda *_: _copy_winsize(master_fd))

    watch = [stdin_fd, master_fd]
    try:
        while True:
            try:
                rs, _, _ = select.select(watch, [], [])
            except InterruptedError:
                continue  # SIGWINCH etc.
            if master_fd in rs:
                try:
                    data = os.read(master_fd, 65536)
                except OSError:
                    data = b""
                if not data:
                    break  # child exited / pty closed
                os.write(sys.stdout.fileno(), data)
            if stdin_fd in rs:
                data = os.read(stdin_fd, 65536)
                if not data:
                    # Local stdin EOF (e.g. non-interactive driver): stop
                    # forwarding input but keep relaying the child's
                    # output until IT exits — do NOT kill the child.
                    watch.remove(stdin_fd)
                else:
                    os.write(master_fd, data)
    finally:
        if saved is not None:
            termios.tcsetattr(stdin_fd, termios.TCSADRAIN, saved)

    _, status = os.waitpid(pid, 0)
    if os.WIFSIGNALED(status):
        return 128 + os.WTERMSIG(status)
    return os.WEXITSTATUS(status)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
