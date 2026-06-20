//! Driving kernel build steps after a `.config` change.
//!
//! patchkc never reimplements `make`/`kbuild`; it just shells out with a
//! predictable, logged `make -C <kernel_src> <target>` invocation so the
//! caller doesn't need to `cd` into the kernel tree themselves.

use std::path::Path;
use std::process::Command;

use crate::utils::error::{PatchError, Result};
use crate::utils::logger;

/// Steps patchkc can drive in the kernel source tree after patching
/// `.config`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStep {
    /// Resolve a hand-edited `.config` against the current Kconfig tree,
    /// taking the default for anything newly introduced.
    OldDefConfig,
    /// Build all enabled modules.
    Modules,
    /// Install built modules into `/lib/modules/$(uname -r)`.
    ModulesInstall,
}

impl BuildStep {
    fn make_target(&self) -> &'static str {
        match self {
            BuildStep::OldDefConfig => "olddefconfig",
            BuildStep::Modules => "modules",
            BuildStep::ModulesInstall => "modules_install",
        }
    }
}

/// Run `make -C <kernel_src> [-jN] <step's target>`.
pub fn run_step(kernel_src: &Path, step: BuildStep, jobs: Option<usize>, dry_run: bool) -> Result<()> {
    let target = step.make_target();

    let mut cmd = Command::new("make");
    cmd.arg("-C").arg(kernel_src);
    if let Some(j) = jobs {
        cmd.arg(format!("-j{j}"));
    }
    cmd.arg(target);

    if dry_run {
        logger::info(format!(
            "[dry-run] would run: make -C {} {}",
            kernel_src.display(),
            target
        ));
        return Ok(());
    }

    logger::info(format!("running: make -C {} {target}", kernel_src.display()));
    let status = cmd.status().map_err(|e| PatchError::Command {
        cmd: format!("make {target}"),
        detail: e.to_string(),
    })?;

    if status.success() {
        logger::ok(format!("`make {target}` finished"));
        Ok(())
    } else {
        Err(PatchError::Command {
            cmd: format!("make {target}"),
            detail: format!("exited with {status}"),
        })
    }
}
