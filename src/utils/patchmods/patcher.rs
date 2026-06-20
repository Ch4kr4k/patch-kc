//! Applying a patch config's option values onto a kernel config, in memory.
//!
//! Writing the result to disk and backing up the original are the caller's
//! responsibility (see `kernel_patcher::cmd_apply`); this module is pure
//! data manipulation so it's trivial to unit test and to dry-run.

use crate::utils::kconfig::KernelConfig;

/// What [`apply`] did, so callers can report it before/instead of writing
/// it to disk.
#[derive(Debug, Default, Clone)]
pub struct ApplyReport {
    /// `(key, old_value, new_value)` for options that existed and changed.
    pub changed: Vec<(String, String, String)>,
    /// `(key, new_value)` for options the patch config introduced that the
    /// kernel config didn't have at all.
    pub added: Vec<(String, String)>,
}

impl ApplyReport {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }

    pub fn total(&self) -> usize {
        self.changed.len() + self.added.len()
    }
}

/// Apply every option in `patch` onto `kernel`, mutating it in place.
/// Options already at the desired value are left untouched.
pub fn apply(kernel: &mut KernelConfig, patch: &KernelConfig) -> ApplyReport {
    let mut report = ApplyReport::default();
    let keys: Vec<String> = patch.keys().map(str::to_string).collect();

    for key in keys {
        let patch_state = patch
            .get(&key)
            .expect("key was just read from patch.keys()")
            .clone();

        match kernel.get(&key).cloned() {
            Some(existing) if existing == patch_state => {}
            Some(existing) => {
                report
                    .changed
                    .push((key.clone(), existing.as_display(), patch_state.as_display()));
                kernel.set(&key, patch_state);
            }
            None => {
                report.added.push((key.clone(), patch_state.as_display()));
                kernel.set(&key, patch_state);
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_changes_and_additions_only() {
        let mut kernel = KernelConfig::parse("CONFIG_A=y\nCONFIG_B=m\n");
        let patch = KernelConfig::parse("CONFIG_A=y\nCONFIG_B=n\nCONFIG_C=y\n");

        let report = apply(&mut kernel, &patch);

        assert_eq!(report.changed, vec![("B".to_string(), "m".to_string(), "n".to_string())]);
        assert_eq!(report.added, vec![("C".to_string(), "y".to_string())]);
        assert_eq!(kernel.render(), "CONFIG_A=y\nCONFIG_B=n\nCONFIG_C=y\n");
    }

    #[test]
    fn no_op_when_already_matching() {
        let mut kernel = KernelConfig::parse("CONFIG_A=y\n");
        let patch = KernelConfig::parse("CONFIG_A=y\n");
        let report = apply(&mut kernel, &patch);
        assert!(report.is_empty());
    }
}
