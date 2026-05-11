//! Workspace state and path sandboxing.
//!
//! The "workspace" is the user-chosen root directory that scopes every
//! filesystem tool. It is held in memory by the plugin and persisted to a
//! small JSON file in the Tauri app-data directory so it survives restarts.
//!
//! Every tool command MUST run its caller-supplied path through `resolve`,
//! which canonicalizes the result and refuses anything that escapes the
//! workspace root.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::RwLock;

use crate::Error;

const STATE_FILENAME: &str = "co_worker_workspace.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedWorkspace {
    root: Option<PathBuf>,
}

/// In-memory workspace state. Wrapped in an `RwLock` so the agent loop can
/// read it concurrently with the frontend updating it.
#[derive(Default)]
pub struct WorkspaceState {
    inner: RwLock<Option<PathBuf>>,
}

impl WorkspaceState {
    pub async fn get(&self) -> Option<PathBuf> {
        self.inner.read().await.clone()
    }

    pub async fn set(&self, path: Option<PathBuf>) {
        *self.inner.write().await = path;
    }
}

/// Resolve a workspace-relative (or absolute, as long as it stays inside)
/// path to its canonical absolute form, verifying it lives under `root`.
///
/// Returns the canonical path on success. The `must_exist` flag controls
/// whether we require the target to already exist:
///
/// - `true`  — used by reads / deletes / moves. Canonicalizes against the
///   actual filesystem.
/// - `false` — used by writes / `create_dir`. The target may not exist
///   yet; we canonicalize its parent and rejoin the final component.
pub fn resolve(root: &Path, requested: &str, must_exist: bool) -> Result<PathBuf, Error> {
    if requested.contains('\0') {
        return Err(Error::ToolBadPath("path contains NUL byte".into()));
    }
    let candidate = if Path::new(requested).is_absolute() {
        PathBuf::from(requested)
    } else {
        root.join(requested)
    };

    let canonical = if must_exist {
        candidate.canonicalize().map_err(|e| {
            Error::ToolBadPath(format!("could not resolve `{}`: {e}", requested))
        })?
    } else {
        let parent = candidate.parent().ok_or_else(|| {
            Error::ToolBadPath(format!("`{requested}` has no parent directory"))
        })?;
        let parent_canon = if parent.as_os_str().is_empty() {
            root.canonicalize()
                .map_err(|e| Error::ToolBadPath(format!("workspace root invalid: {e}")))?
        } else {
            parent.canonicalize().map_err(|e| {
                Error::ToolBadPath(format!(
                    "parent of `{requested}` does not exist: {e}"
                ))
            })?
        };
        match candidate.file_name() {
            Some(name) => parent_canon.join(name),
            None => parent_canon,
        }
    };

    let root_canon = root
        .canonicalize()
        .map_err(|e| Error::ToolBadPath(format!("workspace root invalid: {e}")))?;

    if !canonical.starts_with(&root_canon) {
        return Err(Error::ToolOutsideWorkspace(
            requested.to_string(),
            root_canon.display().to_string(),
        ));
    }
    Ok(canonical)
}

/// Render a path relative to the workspace root, falling back to the
/// canonical form on failure. Used in tool-result strings the model sees.
pub fn display_relative(root: &Path, full: &Path) -> String {
    full.strip_prefix(root)
        .ok()
        .and_then(|r| r.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| full.display().to_string())
}

/// Load the persisted workspace at startup. Errors are logged and swallowed
/// — a missing/corrupt state file is not fatal.
pub async fn load_persisted<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    let path = state_path(app)?;
    let bytes = tokio::fs::read(&path).await.ok()?;
    let parsed: PersistedWorkspace = serde_json::from_slice(&bytes).ok()?;
    parsed.root.filter(|p| p.exists())
}

/// Persist the current workspace selection. Best-effort; errors are
/// surfaced as `Error::Io` for the caller to log but the workspace state
/// in memory is the authoritative source.
pub async fn save_persisted<R: Runtime>(
    app: &AppHandle<R>,
    root: Option<&Path>,
) -> Result<(), Error> {
    let Some(path) = state_path(app) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(Error::Io)?;
    }
    let payload = PersistedWorkspace {
        root: root.map(PathBuf::from),
    };
    let bytes = serde_json::to_vec_pretty(&payload).map_err(Error::Json)?;
    tokio::fs::write(&path, bytes).await.map_err(Error::Io)?;
    Ok(())
}

fn state_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join(STATE_FILENAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn refuses_escape_via_dotdot() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let result = resolve(&root, "../etc/passwd", false);
        // Either "outside workspace" (resolved successfully but escaped)
        // or "bad path" (parent didn't exist) is acceptable — both refuse.
        assert!(matches!(
            result,
            Err(Error::ToolOutsideWorkspace(..)) | Err(Error::ToolBadPath(..))
        ));
    }

    #[test]
    fn refuses_absolute_outside_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        // /etc exists on macOS so it canonicalizes; the escape check then
        // fires. On a system where /etc isn't present we'd fall through to
        // ToolBadPath — accept either.
        let result = resolve(&root, "/etc/passwd", false);
        assert!(matches!(
            result,
            Err(Error::ToolOutsideWorkspace(..)) | Err(Error::ToolBadPath(..))
        ));
    }

    #[test]
    fn accepts_normal_relative_path() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::write(root.join("a.txt"), "hi").unwrap();
        let resolved = resolve(&root, "a.txt", true).unwrap();
        assert_eq!(resolved, root.join("a.txt"));
    }

    #[test]
    fn write_target_can_be_new_file_with_existing_parent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let resolved = resolve(&root, "new.txt", false).unwrap();
        assert_eq!(resolved, root.join("new.txt"));
    }
}
