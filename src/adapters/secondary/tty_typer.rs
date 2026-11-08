//! Types a line into the controlling terminal's input queue with `TIOCSTI`
//! (ADR-2610281500 §2), so the shell that started ccs reads it, runs it and
//! files it in history. Some kernels (Linux 6.2+) refuse; then it says no.

use crate::ports::Typer;

pub struct TtyTyper;

impl Typer for TtyTyper {
    fn type_line(&self, line: &str) -> bool {
        if line.chars().any(char::is_control) {
            return false;
        }
        let fd = libc::STDIN_FILENO;
        // SAFETY: isatty only reads the descriptor's kind.
        if unsafe { libc::isatty(fd) } != 1 {
            return false;
        }
        for b in line.bytes().chain(std::iter::once(b'\n')) {
            // SAFETY: TIOCSTI takes a pointer to one byte, valid for the call.
            if unsafe { libc::ioctl(fd, libc::TIOCSTI, &b as *const u8) } != 0 {
                return false;
            }
        }
        true
    }
}
