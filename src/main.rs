use clap::{Parser};
mod utils;
use utils::kernel_patcher::new_kernel_patcher;
use utils::argparser::Args;

fn main() {
    let args = Args::parse();
    let mut kp = new_kernel_patcher(args);
    kp.run();
}