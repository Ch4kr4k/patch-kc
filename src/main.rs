mod utils;

use std::process::ExitCode;

use clap::Parser;

use utils::argparser::Args;
use utils::kernel_patcher::new_kernel_patcher;
use utils::logger;

/// Restore the default SIGPIPE disposition.
///
/// Rust ignores SIGPIPE by default, which turns "downstream pipe closed
/// early" (e.g. `patchkc module list | head`) into an `std::io::Error` that
/// `println!`/`writeln!` then *panics* on instead of the process quietly
/// exiting the way `ls | head` does. Resetting it to `SIG_DFL` gives patchkc
/// normal Unix pipe behaviour.
fn reset_sigpipe() {
    unsafe {
        let _ = nix::sys::signal::signal(nix::sys::signal::Signal::SIGPIPE, nix::sys::signal::SigHandler::SigDfl);
    }
}

fn main() -> ExitCode {
    reset_sigpipe();
    let args = Args::parse();
    let mut kp = new_kernel_patcher(args);

    match kp.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            logger::error(e.to_string());
            ExitCode::FAILURE
        }
    }
}
