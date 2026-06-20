//! Parsing, in-memory representation, and format-preserving serialisation of
//! Linux kernel `.config` files (Kconfig/Kbuild output).
//!
//! The kernel's config format is a flat list of lines of three kinds:
//!
//! * `CONFIG_FOO=y` / `=m` / `=<string|number>` -- an enabled option
//! * `# CONFIG_FOO is not set`                   -- an explicitly disabled option
//! * everything else (blank lines, `# comment`, section banners such as
//!   `# end of Foo`) -- preserved verbatim and never touched by patchkc
//!
//! [`KernelConfig`] keeps the original line order and verbatim text for
//! every comment/banner, so writing a patched file back out produces a
//! minimal, human-reviewable diff against the original instead of a fully
//! reformatted file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::error::{PatchError, Result};

/// A single option's state, as it would appear in a `.config` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionState {
    /// `CONFIG_FOO=<raw>`, where `raw` is the exact right-hand side text
    /// (`y`, `m`, `"some string"`, `0x1000`, ...) -- preserved as-is so we
    /// never mangle quoting or numeric formatting.
    Set(String),
    /// `# CONFIG_FOO is not set`.
    Unset,
}

impl OptionState {
    /// `y` / `m` / `0x1000` / `not set`, suitable for human-facing output.
    pub fn as_display(&self) -> String {
        match self {
            OptionState::Set(v) => v.clone(),
            OptionState::Unset => "not set".to_string(),
        }
    }

    /// True for `y` or `m` (i.e. "built in" or "module").
    pub fn is_enabled(&self) -> bool {
        matches!(self, OptionState::Set(v) if v == "y" || v == "m")
    }

    /// True specifically for `m` (built as a loadable module).
    pub fn is_module(&self) -> bool {
        matches!(self, OptionState::Set(v) if v == "m")
    }
}

#[derive(Debug, Clone)]
enum Line {
    /// An option in either state, keyed by its name without the `CONFIG_`
    /// prefix.
    Option { key: String, state: OptionState },
    /// Anything else: blank lines, free-form comments, section banners.
    /// Preserved byte-for-byte.
    Verbatim(String),
}

/// An in-memory, order-preserving representation of a kernel `.config` file.
#[derive(Debug, Clone, Default)]
pub struct KernelConfig {
    lines: Vec<Line>,
    index: HashMap<String, usize>,
    pub path: Option<PathBuf>,
}

/// Accept either `CONFIG_FOO` or `FOO` from callers.
fn normalize_key(key: &str) -> &str {
    key.strip_prefix("CONFIG_").unwrap_or(key)
}

impl KernelConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a `.config`-formatted file from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|e| PatchError::io(path, e))?;
        let mut cfg = Self::parse(&text);
        cfg.path = Some(path.to_path_buf());
        Ok(cfg)
    }

    /// Parse `.config` text without requiring a backing file. Used by tests
    /// and anywhere an in-memory patch set is convenient.
    pub fn parse(text: &str) -> Self {
        let mut cfg = KernelConfig::new();
        for raw in text.lines() {
            let trimmed = raw.trim();

            if let Some(rest) = trimmed.strip_prefix("# CONFIG_") {
                if let Some(key) = rest.strip_suffix(" is not set") {
                    cfg.push_option(key.to_string(), OptionState::Unset);
                    continue;
                }
            }

            if let Some(rest) = trimmed.strip_prefix("CONFIG_") {
                if let Some((key, value)) = rest.split_once('=') {
                    cfg.push_option(key.trim().to_string(), OptionState::Set(value.trim().to_string()));
                    continue;
                }
            }

            cfg.lines.push(Line::Verbatim(raw.to_string()));
        }
        cfg
    }

    fn push_option(&mut self, key: String, state: OptionState) {
        if let Some(&idx) = self.index.get(&key) {
            self.lines[idx] = Line::Option { key, state };
        } else {
            self.index.insert(key.clone(), self.lines.len());
            self.lines.push(Line::Option { key, state });
        }
    }

    /// Look up an option's current state. Accepts the key with or without
    /// its `CONFIG_` prefix.
    pub fn get(&self, key: &str) -> Option<&OptionState> {
        let key = normalize_key(key);
        self.index.get(key).and_then(|&i| match &self.lines[i] {
            Line::Option { state, .. } => Some(state),
            Line::Verbatim(_) => None,
        })
    }

    /// All known option keys (without the `CONFIG_` prefix), in file order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().filter_map(|l| match l {
            Line::Option { key, .. } => Some(key.as_str()),
            Line::Verbatim(_) => None,
        })
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Set (or insert) an option's state in place. New options are appended
    /// at the end of the file. Returns `true` if this changed an existing
    /// value.
    pub fn set(&mut self, key: &str, state: OptionState) -> bool {
        let key = normalize_key(key).to_string();
        if let Some(&idx) = self.index.get(&key) {
            let changed = matches!(&self.lines[idx], Line::Option { state: existing, .. } if *existing != state);
            self.lines[idx] = Line::Option { key, state };
            changed
        } else {
            self.index.insert(key.clone(), self.lines.len());
            self.lines.push(Line::Option { key, state });
            false
        }
    }

    /// Render back to `.config` text, preserving original line order and
    /// verbatim comments/banners.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.lines.len() * 24);
        for line in &self.lines {
            match line {
                Line::Verbatim(raw) => {
                    out.push_str(raw);
                }
                Line::Option { key, state } => match state {
                    OptionState::Set(v) => {
                        out.push_str("CONFIG_");
                        out.push_str(key);
                        out.push('=');
                        out.push_str(v);
                    }
                    OptionState::Unset => {
                        out.push_str("# CONFIG_");
                        out.push_str(key);
                        out.push_str(" is not set");
                    }
                },
            }
            out.push('\n');
        }
        out
    }

    /// Write the current state back to `path`, replacing its contents.
    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        fs::write(path, self.render()).map_err(|e| PatchError::io(path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lines() {
        let cfg = KernelConfig::parse(
            "CONFIG_FOO=y\n# CONFIG_BAR is not set\nCONFIG_NAME=\"hello world\"\n# a comment\n\n",
        );
        assert_eq!(cfg.get("FOO"), Some(&OptionState::Set("y".into())));
        assert_eq!(cfg.get("CONFIG_FOO"), Some(&OptionState::Set("y".into())));
        assert_eq!(cfg.get("BAR"), Some(&OptionState::Unset));
        assert_eq!(
            cfg.get("NAME"),
            Some(&OptionState::Set("\"hello world\"".into()))
        );
        assert_eq!(cfg.get("MISSING"), None);
    }

    #[test]
    fn round_trips_verbatim_comments() {
        let text = "# Top banner\nCONFIG_FOO=y\n# end of Top banner\n";
        let cfg = KernelConfig::parse(text);
        assert_eq!(cfg.render(), text);
    }

    #[test]
    fn set_updates_in_place_without_reordering() {
        let mut cfg = KernelConfig::parse("CONFIG_A=y\nCONFIG_B=m\n");
        let changed = cfg.set("A", OptionState::Set("n".into()));
        // last entry of CONFIG_A is overwritten with "n" (not a real kernel
        // value, but exercises the in-place update path)
        assert!(changed);
        assert_eq!(cfg.render(), "CONFIG_A=n\nCONFIG_B=m\n");
    }

    #[test]
    fn set_appends_new_keys() {
        let mut cfg = KernelConfig::parse("CONFIG_A=y\n");
        let changed = cfg.set("B", OptionState::Set("m".into()));
        assert!(!changed);
        assert_eq!(cfg.render(), "CONFIG_A=y\nCONFIG_B=m\n");
    }

    #[test]
    fn section_banners_are_not_options() {
        let cfg = KernelConfig::parse("# Slab allocator options\nCONFIG_SLUB=y\n");
        assert_eq!(cfg.keys().count(), 1);
        assert_eq!(cfg.get("Slab allocator options"), None);
    }
}
