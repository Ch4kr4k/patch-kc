//! Centralised constants and defaults used throughout patchkc.

/// Default path to the currently configured kernel's `.config` file.
pub const KERNEL_CONFIG_PATH: &str = "/usr/src/linux/.config";

/// Default root of the kernel source tree, used for `make` invocations.
pub const KERNEL_SRC_DIR: &str = "/usr/src/linux";

/// Where timestamped backups of patched files are kept by default.
pub const DEFAULT_BACKUP_DIR: &str = "/var/backups/patchkc";

/// Kernel-exposed table of currently loaded modules.
pub const PROC_MODULES_PATH: &str = "/proc/modules";

/// Extension appended to backup files, after a unix-timestamp component.
pub const BACKUP_SUFFIX: &str = "bak";
