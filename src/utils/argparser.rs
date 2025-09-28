use clap::{Parser};
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the user config file
    #[arg(short = 'c', long = "config")]
    pub config: String,

    #[arg(short = 'd', long = "diff")]
    pub diff: bool,
}