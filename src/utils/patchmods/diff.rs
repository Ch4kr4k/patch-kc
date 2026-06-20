//! Comparing a kernel `.config` against a desired "patch" config.

use crate::utils::kconfig::KernelConfig;
use crate::utils::logger;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffEntry {
    /// Present in both, with the same value.
    Matched { key: String, value: String },
    /// Present in both, with different values.
    Changed {
        key: String,
        kernel: String,
        patch: String,
    },
    /// Only present in the patch config.
    OnlyInPatch { key: String, value: String },
}

impl DiffEntry {
    /// The `CONFIG_` key this entry is about, regardless of variant.
    pub fn key(&self) -> &str {
        match self {
            DiffEntry::Matched { key, .. }
            | DiffEntry::Changed { key, .. }
            | DiffEntry::OnlyInPatch { key, .. } => key,
        }
    }
}

/// Compare every option set in `patch` against `kernel`.
pub fn diff(kernel: &KernelConfig, patch: &KernelConfig) -> Vec<DiffEntry> {
    let mut out = Vec::new();
    for key in patch.keys() {
        let patch_state = patch.get(key).expect("key came from patch.keys()");
        match kernel.get(key) {
            Some(kernel_state) if kernel_state == patch_state => {
                out.push(DiffEntry::Matched {
                    key: key.to_string(),
                    value: kernel_state.as_display(),
                });
            }
            Some(kernel_state) => {
                out.push(DiffEntry::Changed {
                    key: key.to_string(),
                    kernel: kernel_state.as_display(),
                    patch: patch_state.as_display(),
                });
            }
            None => {
                out.push(DiffEntry::OnlyInPatch {
                    key: key.to_string(),
                    value: patch_state.as_display(),
                });
            }
        }
    }
    out
}

/// Print a diff report to stdout/stderr via [`logger`].
pub fn print_diff(entries: &[DiffEntry], show_matched: bool) {
    for entry in entries {
        match entry {
            DiffEntry::Matched { key, value } => {
                if show_matched {
                    logger::ok(format!("CONFIG_{key} matches (`{value}`)"));
                }
            }
            DiffEntry::Changed { key, kernel, patch } => {
                logger::warn(format!("CONFIG_{key}: kernel=`{kernel}` patch=`{patch}`"));
            }
            DiffEntry::OnlyInPatch { key, value } => {
                logger::info(format!("CONFIG_{key}: only in patch (`{value}`)"));
            }
        }
    }
}

/// `(matched, changed, only_in_patch)` counts.
pub fn summary(entries: &[DiffEntry]) -> (usize, usize, usize) {
    entries.iter().fold((0, 0, 0), |(m, c, a), e| match e {
        DiffEntry::Matched { .. } => (m + 1, c, a),
        DiffEntry::Changed { .. } => (m, c + 1, a),
        DiffEntry::OnlyInPatch { .. } => (m, c, a + 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_matched_changed_and_added() {
        let kernel = KernelConfig::parse("CONFIG_A=y\nCONFIG_B=m\n");
        let patch = KernelConfig::parse("CONFIG_A=y\nCONFIG_B=n\nCONFIG_C=y\n");
        let entries = diff(&kernel, &patch);
        assert_eq!(summary(&entries), (1, 1, 1));
    }
}
