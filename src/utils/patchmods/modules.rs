//! Inspecting and (un)loading kernel modules.
//!
//! Listing/status reads `/proc/modules` directly. Loading/unloading
//! prefers the raw `finit_module(2)`/`delete_module(2)` syscalls (via
//! `nix::kmod`) and falls back to the `modprobe`/`insmod`/`rmmod` userspace
//! tools, which additionally resolve module dependencies -- something the
//! bare syscalls deliberately don't do.

use std::ffi::CString;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;

use nix::kmod::{delete_module, finit_module, DeleteModuleFlags, ModuleInitFlags};

use crate::utils::consts::PROC_MODULES_PATH;
use crate::utils::error::{PatchError, Result};
use crate::utils::logger;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInfo {
    pub name: String,
    pub size: u64,
    pub use_count: i64,
    pub used_by: Vec<String>,
    pub state: String,
}

/// Parse `/proc/modules` to list every module currently loaded.
pub fn list_loaded() -> Result<Vec<ModuleInfo>> {
    list_loaded_from(PROC_MODULES_PATH)
}

fn list_loaded_from(path: impl AsRef<Path>) -> Result<Vec<ModuleInfo>> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|e| PatchError::io(path, e))?;
    let reader = BufReader::new(file);
    let mut modules = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| PatchError::io(path, e))?;
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }

        let used_by = fields[3]
            .trim_end_matches(',')
            .split(',')
            .filter(|s| !s.is_empty() && *s != "-")
            .map(str::to_string)
            .collect();

        modules.push(ModuleInfo {
            name: fields[0].to_string(),
            size: fields[1].parse().unwrap_or(0),
            use_count: fields[2].parse().unwrap_or(0),
            used_by,
            state: fields[4].to_string(),
        });
    }
    Ok(modules)
}

pub fn is_loaded(name: &str) -> Result<bool> {
    Ok(list_loaded()?.iter().any(|m| m.name == name))
}

/// Load a module, either by name (resolved via `modprobe`, which also
/// pulls in dependencies) or by path to a `.ko`/`.ko.xz`/`.ko.zst`/`.ko.gz`
/// file (loaded directly via `finit_module(2)`, falling back to `insmod`
/// if that fails).
pub fn load(spec: &str, params: &[String], dry_run: bool) -> Result<()> {
    if dry_run {
        logger::info(format!("[dry-run] would load module `{spec}`"));
        return Ok(());
    }

    let looks_like_file = spec.contains('/')
        || spec.ends_with(".ko")
        || spec.ends_with(".ko.xz")
        || spec.ends_with(".ko.zst")
        || spec.ends_with(".ko.gz");

    if looks_like_file {
        load_from_file(Path::new(spec), params)
    } else {
        load_by_name(spec, params)
    }
}

fn load_by_name(name: &str, params: &[String]) -> Result<()> {
    let mut cmd = Command::new("modprobe");
    cmd.arg(name).args(params);
    run_checked(cmd, "modprobe")?;
    logger::ok(format!("loaded `{name}` via modprobe"));
    Ok(())
}

fn load_from_file(path: &Path, params: &[String]) -> Result<()> {
    let param_str = params.join(" ");
    let c_params =
        CString::new(param_str).map_err(|e| PatchError::Module(format!("invalid module params: {e}")))?;

    let file = File::open(path).map_err(|e| PatchError::io(path, e))?;
    match finit_module(&file, &c_params, ModuleInitFlags::empty()) {
        Ok(()) => {
            logger::ok(format!("loaded `{}` via finit_module(2)", path.display()));
            Ok(())
        }
        Err(errno) => {
            logger::debug(format!(
                "finit_module(2) failed for `{}` ({errno}); falling back to insmod",
                path.display()
            ));
            let mut cmd = Command::new("insmod");
            cmd.arg(path).args(params);
            run_checked(cmd, "insmod")?;
            logger::ok(format!("loaded `{}` via insmod", path.display()));
            Ok(())
        }
    }
}

/// Unload a module by name, preferring the raw `delete_module(2)` syscall
/// and falling back to `modprobe -r` (which also unwinds now-unused
/// dependents) if the syscall path fails.
pub fn unload(name: &str, force: bool, dry_run: bool) -> Result<()> {
    if dry_run {
        logger::info(format!("[dry-run] would unload module `{name}`"));
        return Ok(());
    }

    let c_name =
        CString::new(name).map_err(|e| PatchError::Module(format!("invalid module name: {e}")))?;
    let flags = if force {
        DeleteModuleFlags::O_NONBLOCK | DeleteModuleFlags::O_TRUNC
    } else {
        DeleteModuleFlags::O_NONBLOCK
    };

    match delete_module(&c_name, flags) {
        Ok(()) => {
            logger::ok(format!("unloaded `{name}`"));
            Ok(())
        }
        Err(errno) => {
            logger::debug(format!(
                "delete_module(2) failed for `{name}` ({errno}); falling back to `modprobe -r`"
            ));
            let mut cmd = Command::new("modprobe");
            cmd.arg("-r").arg(name);
            run_checked(cmd, "modprobe -r")?;
            logger::ok(format!("unloaded `{name}` via modprobe -r"));
            Ok(())
        }
    }
}

fn run_checked(mut cmd: Command, label: &str) -> Result<()> {
    let output = cmd.output().map_err(|e| PatchError::Command {
        cmd: label.to_string(),
        detail: e.to_string(),
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PatchError::Command {
            cmd: label.to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_proc_modules_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("modules");
        fs::write(
            &path,
            "nvidia 12345678 3 nvidia_uvm,nvidia_drm, Live 0x0000000000000000\n\
             tun 28672 0 - Live 0x0000000000000000\n",
        )
        .unwrap();

        let modules = list_loaded_from(&path).unwrap();
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].name, "nvidia");
        assert_eq!(modules[0].used_by, vec!["nvidia_uvm", "nvidia_drm"]);
        assert_eq!(modules[1].name, "tun");
        assert!(modules[1].used_by.is_empty());
    }

    #[test]
    fn is_loaded_checks_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("modules");
        fs::write(&path, "tun 28672 0 - Live 0x0\n").unwrap();
        let modules = list_loaded_from(&path).unwrap();
        assert!(modules.iter().any(|m| m.name == "tun"));
        assert!(!modules.iter().any(|m| m.name == "missing"));
    }
}
