//! Top-level orchestration: maps a parsed [`Args`] onto the library calls
//! in `kconfig`/`backup`/`patchmods::*`, handling root checks and
//! interactive confirmation along the way.

use std::io::{self, Write};
use std::path::Path;

use crate::utils::argparser::{Args, Command, ModuleAction};
use crate::utils::backup;
use crate::utils::error::{PatchError, Result};
use crate::utils::kconfig::KernelConfig;
use crate::utils::logger;
use crate::utils::patchmods::{build, diff, modules, patcher};

pub struct KernelPatcher {
    args: Args,
}

impl KernelPatcher {
    pub fn run(&mut self) -> Result<()> {
        logger::set_verbosity(self.args.verbose);
        logger::trace(format!("dispatching command: {:?}", self.args.command));

        match self.args.command.clone() {
            Command::Diff {
                config,
                show_matched,
                only_modules,
                only_enabled,
            } => self.cmd_diff(&config, show_matched, only_modules, only_enabled),
            Command::Apply {
                config,
                no_backup,
                backup_dir,
            } => {
                self.require_root()?;
                self.cmd_apply(&config, no_backup, &backup_dir)
            }
            Command::Backup { backup_dir } => {
                self.require_root()?;
                self.cmd_backup(&backup_dir)
            }
            Command::Restore { from, backup_dir } => {
                self.require_root()?;
                self.cmd_restore(from.as_deref(), &backup_dir)
            }
            Command::Module { action } => self.cmd_module(&action),
            Command::Build {
                kernel_src,
                oldconfig,
                modules: build_modules,
                install,
                jobs,
            } => {
                self.require_root()?;
                self.cmd_build(&kernel_src, oldconfig, build_modules, install, jobs)
            }
        }
    }

    fn require_root(&self) -> Result<()> {
        if nix::unistd::Uid::effective().is_root() {
            Ok(())
        } else {
            Err(PatchError::NotRoot)
        }
    }

    /// Ask the user to confirm an action, unless `--yes` or `--dry-run` was
    /// passed (a dry run never mutates anything, so there's nothing to
    /// confirm).
    fn confirm(&self, prompt: &str) -> bool {
        if self.args.yes || self.args.dry_run {
            return true;
        }
        print!("{prompt} [y/N] ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            return false;
        }
        matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
    }

    fn cmd_diff(&self, patch_config: &Path, show_matched: bool, only_modules: bool, only_enabled: bool) -> Result<()> {
        let kernel = KernelConfig::load(&self.args.kernel_config)?;
        let patch = load_patch_config(patch_config)?;
        logger::debug(format!(
            "kernel config: {} options, patch config: {} options",
            kernel.len(),
            patch.len()
        ));

        let mut entries = diff::diff(&kernel, &patch);
        filter_by_patch_state(&mut entries, &patch, only_modules, only_enabled);
        diff::print_diff(&entries, show_matched);

        let (matched, changed, added) = diff::summary(&entries);
        logger::info(format!("{matched} matched, {changed} differ, {added} only in patch"));
        Ok(())
    }

    fn cmd_apply(&self, patch_config: &Path, no_backup: bool, backup_dir: &Path) -> Result<()> {
        let mut kernel = KernelConfig::load(&self.args.kernel_config)?;
        let patch = load_patch_config(patch_config)?;

        let report = patcher::apply(&mut kernel, &patch);
        if report.is_empty() {
            logger::ok("kernel config already matches the patch config; nothing to do");
            return Ok(());
        }

        for (key, old, new) in &report.changed {
            logger::warn(format!("CONFIG_{key}: `{old}` -> `{new}`"));
        }
        for (key, new) in &report.added {
            logger::info(format!("CONFIG_{key}: (absent) -> `{new}` [new]"));
        }

        if self.args.dry_run {
            logger::info(format!("dry-run: {} change(s) not written", report.total()));
            return Ok(());
        }

        if !self.confirm(&format!(
            "Apply {} change(s) to `{}`?",
            report.total(),
            self.args.kernel_config.display()
        )) {
            logger::warn("aborted by user");
            return Ok(());
        }

        if !no_backup {
            let bpath = backup::create(&self.args.kernel_config, backup_dir)?;
            logger::ok(format!("backed up current config to `{}`", bpath.display()));
        }

        kernel.write_to(&self.args.kernel_config)?;
        logger::ok(format!("wrote `{}`", self.args.kernel_config.display()));
        Ok(())
    }

    fn cmd_backup(&self, backup_dir: &Path) -> Result<()> {
        let path = backup::create(&self.args.kernel_config, backup_dir)?;
        logger::ok(format!(
            "backed up `{}` -> `{}`",
            self.args.kernel_config.display(),
            path.display()
        ));
        Ok(())
    }

    fn cmd_restore(&self, from: Option<&Path>, backup_dir: &Path) -> Result<()> {
        let restored_from = match from {
            Some(p) => {
                backup::restore(p, &self.args.kernel_config, backup_dir)?;
                p.to_path_buf()
            }
            None => backup::restore_latest(&self.args.kernel_config, backup_dir)?,
        };
        logger::ok(format!(
            "restored `{}` from `{}`",
            self.args.kernel_config.display(),
            restored_from.display()
        ));
        Ok(())
    }

    fn cmd_module(&self, action: &ModuleAction) -> Result<()> {
        match action {
            ModuleAction::List => {
                for m in modules::list_loaded()? {
                    let used_by = if m.used_by.is_empty() {
                        "-".to_string()
                    } else {
                        m.used_by.join(",")
                    };
                    println!("{:<24} size={:<10} used_by={used_by}", m.name, m.size);
                }
                Ok(())
            }
            ModuleAction::Status { name } => {
                if modules::is_loaded(name)? {
                    logger::ok(format!("`{name}` is loaded"));
                } else {
                    logger::warn(format!("`{name}` is not loaded"));
                }
                Ok(())
            }
            ModuleAction::Load { module, params } => {
                self.require_root()?;
                modules::load(module, params, self.args.dry_run)
            }
            ModuleAction::Unload { name, force } => {
                self.require_root()?;
                modules::unload(name, *force, self.args.dry_run)
            }
        }
    }

    fn cmd_build(
        &self,
        kernel_src: &Path,
        oldconfig: bool,
        build_modules: bool,
        install: bool,
        jobs: Option<usize>,
    ) -> Result<()> {
        if !oldconfig && !build_modules && !install {
            logger::warn("nothing to do: pass --oldconfig, --modules and/or --install");
            return Ok(());
        }

        if oldconfig {
            build::run_step(kernel_src, build::BuildStep::OldDefConfig, jobs, self.args.dry_run)?;
        }
        if build_modules {
            build::run_step(kernel_src, build::BuildStep::Modules, jobs, self.args.dry_run)?;
        }
        if install {
            if !self.confirm("Install built modules into the live module tree?") {
                logger::warn("skipped modules_install");
                return Ok(());
            }
            build::run_step(kernel_src, build::BuildStep::ModulesInstall, jobs, self.args.dry_run)?;
        }
        Ok(())
    }
}

pub fn new_kernel_patcher(args: Args) -> KernelPatcher {
    KernelPatcher { args }
}

/// Load a patch config and reject empty ones up front -- an empty patch
/// config is almost always a mistake (wrong path, blank file) rather than
/// a deliberate "change nothing" request, so fail loudly instead of
/// silently reporting zero diffs.
fn load_patch_config(path: &Path) -> Result<KernelConfig> {
    let patch = KernelConfig::load(path)?;
    if patch.is_empty() {
        return Err(PatchError::Other(format!(
            "patch config `{}` contains no CONFIG_ options",
            path.display()
        )));
    }
    Ok(patch)
}

/// Narrow a diff report down to only the options that the patch config
/// would build as a module (`only_modules`) and/or enable at all
/// (`only_enabled`). Both filters apply together when both are set.
fn filter_by_patch_state(entries: &mut Vec<diff::DiffEntry>, patch: &KernelConfig, only_modules: bool, only_enabled: bool) {
    if !only_modules && !only_enabled {
        return;
    }
    entries.retain(|e| match patch.get(e.key()) {
        Some(state) => (!only_modules || state.is_module()) && (!only_enabled || state.is_enabled()),
        None => false,
    });
}
