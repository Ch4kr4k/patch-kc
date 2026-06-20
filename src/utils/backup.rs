//! Timestamped backup/restore for files patchkc is about to modify.
//!
//! Every destructive operation (`apply`, `restore` itself) goes through
//! here first, so a bad patch is always one `patchkc restore` away from
//! being undone.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::utils::consts::BACKUP_SUFFIX;
use crate::utils::error::{PatchError, Result};
use crate::utils::logger;

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_string())
}

/// Pick a backup path of the form `<file_name>.<nanos>[-<n>].bak` that does
/// not already exist. Nanosecond resolution alone makes same-second
/// collisions (e.g. two backups taken in quick succession by a script)
/// vanishingly unlikely; the `-<n>` suffix is a belt-and-suspenders
/// fallback so a collision can never silently overwrite an older backup.
fn unique_backup_path(backup_dir: &Path, file_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut candidate = backup_dir.join(format!("{file_name}.{nanos}.{BACKUP_SUFFIX}"));
    let mut suffix = 0u32;
    while candidate.exists() {
        suffix += 1;
        candidate = backup_dir.join(format!("{file_name}.{nanos}-{suffix}.{BACKUP_SUFFIX}"));
    }
    candidate
}

/// Create a timestamped backup of `path` inside `backup_dir`, creating the
/// directory if necessary. Returns the path to the new backup file.
pub fn create(path: &Path, backup_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(backup_dir).map_err(|e| PatchError::io(backup_dir, e))?;

    let backup_path = unique_backup_path(backup_dir, &file_name_of(path));
    fs::copy(path, &backup_path).map_err(|e| PatchError::io(path, e))?;
    logger::debug(format!(
        "backed up `{}` -> `{}`",
        path.display(),
        backup_path.display()
    ));
    Ok(backup_path)
}

/// List backups for `path`'s file name within `backup_dir`, newest first.
pub fn list(path: &Path, backup_dir: &Path) -> Result<Vec<PathBuf>> {
    let prefix = format!("{}.", file_name_of(path));

    let mut entries: Vec<PathBuf> = match fs::read_dir(backup_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().starts_with(&prefix))
                    .unwrap_or(false)
            })
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(PatchError::io(backup_dir, e)),
    };

    // Names are `<file>.<unix-nanos>[-n].bak`, so a lexicographic sort is
    // also chronological as long as the timestamp digit-width stays
    // constant (true for the foreseeable future).
    entries.sort();
    entries.reverse();
    Ok(entries)
}

/// Restore `backup_path` over `target`, after first snapshotting `target`'s
/// current contents into `backup_dir` (so a restore is itself undoable).
pub fn restore(backup_path: &Path, target: &Path, backup_dir: &Path) -> Result<()> {
    if !backup_path.exists() {
        return Err(PatchError::BackupNotFound(backup_path.to_path_buf()));
    }
    if target.exists() {
        create(target, backup_dir)?;
    }
    fs::copy(backup_path, target).map_err(|e| PatchError::io(target, e))?;
    Ok(())
}

/// Restore the most recent backup for `target`. Returns the backup path
/// that was used.
pub fn restore_latest(target: &Path, backup_dir: &Path) -> Result<PathBuf> {
    let backups = list(target, backup_dir)?;
    let latest = backups
        .into_iter()
        .next()
        .ok_or_else(|| PatchError::BackupNotFound(target.to_path_buf()))?;
    restore(&latest, target, backup_dir)?;
    Ok(latest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn backup_then_restore_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(".config");
        let backup_dir = dir.path().join("backups");

        fs::write(&target, "CONFIG_A=y\n").unwrap();
        let b1 = create(&target, &backup_dir).unwrap();
        assert!(b1.exists());

        fs::write(&target, "CONFIG_A=n\n").unwrap();
        let restored_from = restore_latest(&target, &backup_dir).unwrap();
        assert_eq!(restored_from, b1);
        assert_eq!(fs::read_to_string(&target).unwrap(), "CONFIG_A=y\n");
    }

    #[test]
    fn restore_without_backups_errors() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(".config");
        fs::write(&target, "CONFIG_A=y\n").unwrap();
        let backup_dir = dir.path().join("backups");
        assert!(restore_latest(&target, &backup_dir).is_err());
    }
}
