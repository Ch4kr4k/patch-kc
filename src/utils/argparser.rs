use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

use crate::utils::consts;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "patchkc",
    author,
    version,
    about = "Patch, diff, and manage a Linux kernel's .config and modules",
    long_about = None
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,

    /// Path to the kernel's .config file
    #[arg(
        short = 'k',
        long = "kernel-config",
        global = true,
        default_value_os_t = PathBuf::from(consts::KERNEL_CONFIG_PATH)
    )]
    pub kernel_config: PathBuf,

    /// Show what would happen without changing anything
    #[arg(short = 'n', long = "dry-run", global = true)]
    pub dry_run: bool,

    /// Assume "yes" to any confirmation prompt
    #[arg(short = 'y', long = "yes", global = true)]
    pub yes: bool,

    /// Increase output verbosity (-v, -vv)
    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Show differences between the kernel's .config and a patch config
    Diff {
        /// Path to the patch config file
        #[arg(short = 'c', long = "config")]
        config: PathBuf,

        /// Also print options that already match
        #[arg(long)]
        show_matched: bool,

        /// Only consider options the patch config builds as loadable
        /// modules (`=m`)
        #[arg(long)]
        only_modules: bool,

        /// Only consider options the patch config enables at all (`=y` or `=m`)
        #[arg(long)]
        only_enabled: bool,
    },

    /// Apply a patch config's values onto the kernel's .config
    Apply {
        /// Path to the patch config file
        #[arg(short = 'c', long = "config")]
        config: PathBuf,

        /// Skip creating a backup before writing (not recommended)
        #[arg(long)]
        no_backup: bool,

        /// Directory backups are written to
        #[arg(long, default_value_os_t = PathBuf::from(consts::DEFAULT_BACKUP_DIR))]
        backup_dir: PathBuf,
    },

    /// Create a timestamped backup of the kernel .config
    Backup {
        #[arg(long, default_value_os_t = PathBuf::from(consts::DEFAULT_BACKUP_DIR))]
        backup_dir: PathBuf,
    },

    /// Restore the kernel .config from a backup
    Restore {
        /// Specific backup file to restore; defaults to the most recent
        #[arg(short = 'f', long = "from")]
        from: Option<PathBuf>,

        #[arg(long, default_value_os_t = PathBuf::from(consts::DEFAULT_BACKUP_DIR))]
        backup_dir: PathBuf,
    },

    /// Inspect and manage loaded kernel modules
    Module {
        #[command(subcommand)]
        action: ModuleAction,
    },

    /// Drive kernel build steps (olddefconfig / modules / modules_install)
    Build {
        /// Kernel source tree to build in
        #[arg(long, default_value_os_t = PathBuf::from(consts::KERNEL_SRC_DIR))]
        kernel_src: PathBuf,

        /// Reconcile .config with the Kconfig tree before building
        #[arg(long)]
        oldconfig: bool,

        /// Build modules
        #[arg(long)]
        modules: bool,

        /// Install built modules
        #[arg(long)]
        install: bool,

        /// Parallel jobs passed to `make -j`
        #[arg(short = 'j', long)]
        jobs: Option<usize>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ModuleAction {
    /// List currently loaded modules
    List,
    /// Show whether a specific module is loaded
    Status {
        name: String,
    },
    /// Load a module by name (via modprobe) or by path to a .ko file
    Load {
        module: String,
        /// Extra `key=value` parameters passed to the module
        #[arg(trailing_var_arg = true)]
        params: Vec<String>,
    },
    /// Unload a module by name
    Unload {
        name: String,
        /// Force unload even if the module reports a nonzero use count
        #[arg(short = 'f', long)]
        force: bool,
    },
}
