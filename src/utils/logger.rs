//! Minimal leveled console logger.
//!
//! patchkc is a small, single-purpose CLI, so this intentionally avoids
//! pulling in `log`/`env_logger`: a handful of free functions with
//! consistent prefixes and optional ANSI colour (only when the relevant
//! stream is a TTY) is enough, and keeps output stable for scripting/piping.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicU8, Ordering};

static VERBOSITY: AtomicU8 = AtomicU8::new(0);

/// Set the global verbosity level (typically `args.verbose` from clap's
/// `-v`/`-vv` counter).
pub fn set_verbosity(level: u8) {
    VERBOSITY.store(level, Ordering::Relaxed);
}

pub fn verbosity() -> u8 {
    VERBOSITY.load(Ordering::Relaxed)
}

fn paint(code: &str, prefix: &str, tty: bool) -> String {
    if tty {
        format!("\x1b[{code}m{prefix}\x1b[0m")
    } else {
        prefix.to_string()
    }
}

/// A successful, notable outcome (e.g. "wrote file", "module loaded").
pub fn ok(msg: impl AsRef<str>) {
    let tty = std::io::stdout().is_terminal();
    println!("{} {}", paint("32", "[\u{2713}]", tty), msg.as_ref());
}

/// Routine informational output.
pub fn info(msg: impl AsRef<str>) {
    let tty = std::io::stdout().is_terminal();
    println!("{} {}", paint("36", "[+]", tty), msg.as_ref());
}

/// Something the user should pay attention to, but not fatal.
pub fn warn(msg: impl AsRef<str>) {
    let tty = std::io::stderr().is_terminal();
    eprintln!("{} {}", paint("33", "[!]", tty), msg.as_ref());
}

/// A failure. Errors returned from `main` are also routed through this.
pub fn error(msg: impl AsRef<str>) {
    let tty = std::io::stderr().is_terminal();
    eprintln!("{} {}", paint("31", "[x]", tty), msg.as_ref());
}

/// Printed only with `-v` or higher.
pub fn debug(msg: impl AsRef<str>) {
    if verbosity() > 0 {
        eprintln!("[dbg] {}", msg.as_ref());
    }
}

/// Printed only with `-vv` or higher.
pub fn trace(msg: impl AsRef<str>) {
    if verbosity() > 1 {
        eprintln!("[trace] {}", msg.as_ref());
    }
}
