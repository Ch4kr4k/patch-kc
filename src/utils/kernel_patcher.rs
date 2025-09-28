use crate::utils::{argparser::Args, patchmods::libs::diff, utils::read_configs};
use crate::utils::consts;
// use crate::utils::patchmods::libs;

pub struct KernelPatcher {
    args: Args,
    config_path: String,
}

impl KernelPatcher
{
    pub fn run(&mut self) {
        if self.check_root() {
            println!("[✓]Running as root");
            
            self.config_path = self.args.config.clone();
            // self.kernel_config_path = self.args.kernel_config.clone();

            if self.args.diff {
                println!("[+]checking diffs...\n");

                diff(&read_configs(consts::KERNEL_CONFIG_PATH, &self.config_path));
            } 


        } else {
            println!("[x]Not running as root");
        }
    }

    fn check_root(&mut self) -> bool { unsafe { libc::geteuid() == 0 }}
}

pub fn new_kernel_patcher(args: Args) -> KernelPatcher {
    KernelPatcher {
        args: args,
        config_path: String::new(),
    }
}