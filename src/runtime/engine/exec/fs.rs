use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use crate::{Finding, FindingKind};

use super::super::helpers::decode_hex;
use super::ExecCtx;

impl ExecCtx<'_> {
    pub(super) fn replay_host_fs_write(&mut self, path: &str, data: &[u8]) {
        self.replay_host_fs.insert(path.to_string(), data.to_vec());
    }

    pub(super) fn replay_host_fs_read_assert(
        &mut self,
        path: &str,
        expected: &str,
    ) -> Result<(), Finding> {
        let Some(bytes) = self.replay_host_fs.get(path) else {
            return Err(Finding {
                kind: FindingKind::Assertion,
                title: "fs_read_assert".to_string(),
                message: format!(
                    "expected host fs replay data for {path:?}, but none was recorded"
                ),
                location: None,
            });
        };
        let got = String::from_utf8(bytes.clone()).map_err(|_| Finding {
            kind: FindingKind::Assertion,
            title: "fs_read_assert".to_string(),
            message: format!("recorded host fs bytes for {path:?} are not valid utf-8"),
            location: None,
        })?;
        if got != expected {
            return Err(Finding {
                kind: FindingKind::Assertion,
                title: "fs_read_assert".to_string(),
                message: format!("expected {path:?} == {expected:?}, got {got:?}"),
                location: None,
            });
        }
        Ok(())
    }

    pub(super) fn apply_replay_host_fs_snapshot(
        &mut self,
        name: &str,
        entries: &BTreeMap<String, Option<String>>,
    ) -> Result<(), Finding> {
        let mut decoded = BTreeMap::new();
        for (path, value) in entries {
            let bytes = match value {
                Some(hex) => Some(decode_hex(hex).map_err(|message| Finding {
                    kind: FindingKind::Checker,
                    title: "replay_fs_snapshot".to_string(),
                    message: format!(
                        "invalid recorded host fs snapshot bytes for {name:?} {path:?}: {message}"
                    ),
                    location: None,
                })?),
                None => None,
            };
            decoded.insert(path.clone(), bytes.clone());
            match bytes {
                Some(bytes) => {
                    self.replay_host_fs.insert(path.clone(), bytes);
                }
                None => {
                    self.replay_host_fs.remove(path);
                }
            }
        }
        self.replay_host_fs_snapshots
            .insert(name.to_string(), decoded);
        Ok(())
    }

    pub(super) fn apply_replay_host_fs_restore(&mut self, name: &str) -> Result<(), Finding> {
        let Some(snapshot) = self.replay_host_fs_snapshots.get(name).cloned() else {
            return Err(Finding {
                kind: FindingKind::Checker,
                title: "fs_restore_missing_snapshot".to_string(),
                message: format!("missing replay host fs snapshot {name:?}"),
                location: None,
            });
        };
        self.replay_host_fs
            .retain(|path, _| snapshot.contains_key(path));
        for (path, value) in snapshot {
            match value {
                Some(bytes) => {
                    self.replay_host_fs.insert(path, bytes);
                }
                None => {
                    self.replay_host_fs.remove(&path);
                }
            }
        }
        Ok(())
    }

    fn resolve_host_fs_path(&self, raw: &str) -> Result<PathBuf, Finding> {
        let path = Path::new(raw);
        if path.is_absolute() {
            return Err(Finding {
                kind: FindingKind::Checker,
                title: "host_fs_path".to_string(),
                message: format!("host fs path must be relative to cwd root: {raw:?}"),
                location: None,
            });
        }
        for c in path.components() {
            match c {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "host_fs_path".to_string(),
                        message: format!("host fs path escapes cwd root: {raw:?}"),
                        location: None,
                    });
                }
                Component::CurDir | Component::Normal(_) => {}
            }
        }
        Ok(self.host_root.join(path))
    }

    pub(super) fn host_fs_write(&mut self, raw_path: &str, data: &str) -> Result<(), Finding> {
        let resolved = self.resolve_host_fs_path(raw_path)?;
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Finding {
                kind: FindingKind::Assertion,
                title: "host_fs_write".to_string(),
                message: format!("failed to create parent dir for {raw_path:?}: {e}"),
                location: None,
            })?;
        }
        std::fs::write(&resolved, data).map_err(|e| Finding {
            kind: FindingKind::Assertion,
            title: "host_fs_write".to_string(),
            message: format!("failed to write host fs path {raw_path:?}: {e}"),
            location: None,
        })?;
        self.host_fs_touched.insert(resolved);
        Ok(())
    }

    pub(super) fn host_fs_read_assert(
        &mut self,
        raw_path: &str,
        equals: &str,
    ) -> Result<(), Finding> {
        let resolved = self.resolve_host_fs_path(raw_path)?;
        self.host_fs_touched.insert(resolved.clone());
        let got = std::fs::read_to_string(&resolved).map_err(|e| Finding {
            kind: FindingKind::Assertion,
            title: "host_fs_read_assert".to_string(),
            message: format!("failed to read host fs path {raw_path:?}: {e}"),
            location: None,
        })?;
        if got != equals {
            return Err(Finding {
                kind: FindingKind::Assertion,
                title: "host_fs_read_assert".to_string(),
                message: format!("expected {raw_path:?} == {equals:?}, got {got:?}"),
                location: None,
            });
        }
        Ok(())
    }

    pub(super) fn host_fs_snapshot(&mut self, name: &str) -> Result<(), Finding> {
        let mut snap = BTreeMap::new();
        for path in &self.host_fs_touched {
            let value = if path.exists() {
                Some(std::fs::read(path).map_err(|e| Finding {
                    kind: FindingKind::Assertion,
                    title: "host_fs_snapshot".to_string(),
                    message: format!("failed to read host fs path {:?}: {e}", path),
                    location: None,
                })?)
            } else {
                None
            };
            snap.insert(path.clone(), value);
        }
        self.host_fs_snapshots.insert(name.to_string(), snap);
        Ok(())
    }

    pub(super) fn host_fs_restore(&mut self, name: &str) -> Result<(), Finding> {
        let Some(snapshot) = self.host_fs_snapshots.get(name) else {
            return Err(Finding {
                kind: FindingKind::Checker,
                title: "host_fs_restore_missing_snapshot".to_string(),
                message: format!("missing host fs snapshot {name:?}"),
                location: None,
            });
        };

        for path in snapshot.keys() {
            match snapshot.get(path) {
                Some(Some(bytes)) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| Finding {
                            kind: FindingKind::Assertion,
                            title: "host_fs_restore".to_string(),
                            message: format!("failed to create parent dir for {:?}: {e}", path),
                            location: None,
                        })?;
                    }
                    std::fs::write(path, bytes).map_err(|e| Finding {
                        kind: FindingKind::Assertion,
                        title: "host_fs_restore".to_string(),
                        message: format!("failed to restore file {:?}: {e}", path),
                        location: None,
                    })?;
                }
                Some(None) | None => {
                    if path.exists() {
                        std::fs::remove_file(path).map_err(|e| Finding {
                            kind: FindingKind::Assertion,
                            title: "host_fs_restore".to_string(),
                            message: format!("failed to remove restored file {:?}: {e}", path),
                            location: None,
                        })?;
                    }
                }
            }
        }
        let snapshot_paths = snapshot.keys().cloned().collect::<BTreeSet<_>>();
        for path in self.host_fs_touched.iter() {
            if snapshot_paths.contains(path) {
                continue;
            }
            if path.exists() {
                std::fs::remove_file(path).map_err(|e| Finding {
                    kind: FindingKind::Assertion,
                    title: "host_fs_restore".to_string(),
                    message: format!("failed to remove restored file {:?}: {e}", path),
                    location: None,
                })?;
            }
        }
        self.host_fs_touched = snapshot.keys().cloned().collect();
        Ok(())
    }
}
