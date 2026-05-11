//! Filesystem tool implementations. All operations are sandboxed against
//! the active workspace via [`crate::workspace::resolve`]. Anything outside
//! the workspace root is refused with `Error::ToolOutsideWorkspace`.
//!
//! The plugin's `tool_*` Tauri commands are thin async wrappers over these
//! functions, so the same code runs whether the agent loop dispatches or
//! a UI button does.

use std::path::Path;

use serde::Serialize;

use crate::Error;
use crate::workspace::display_relative;

/// Default cap for `read_file`. The model can request larger reads
/// explicitly via `max_bytes` (still bounded by `MAX_READ_BYTES_HARD`).
pub const DEFAULT_READ_BYTES: u64 = 256 * 1024;
pub const MAX_READ_BYTES_HARD: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct DirEntry {
    pub name: String,
    /// `"dir"`, `"file"`, or `"symlink"`.
    pub kind: &'static str,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListDirResult {
    pub path: String,
    pub entries: Vec<DirEntry>,
}

pub async fn list_dir(root: &Path, path: &str) -> Result<ListDirResult, Error> {
    let target = crate::workspace::resolve(root, path, true)?;
    let mut iter = tokio::fs::read_dir(&target).await.map_err(Error::Io)?;
    let mut entries = Vec::new();
    while let Some(entry) = iter.next_entry().await.map_err(Error::Io)? {
        let ft = entry.file_type().await.map_err(Error::Io)?;
        let kind = if ft.is_dir() {
            "dir"
        } else if ft.is_symlink() {
            "symlink"
        } else {
            "file"
        };
        let size_bytes = if ft.is_file() {
            entry.metadata().await.ok().map(|m| m.len())
        } else {
            None
        };
        entries.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind,
            size_bytes,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ListDirResult {
        path: display_relative(root, &target),
        entries,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadFileResult {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub bytes_read: u64,
}

pub async fn read_file(
    root: &Path,
    path: &str,
    max_bytes: Option<u64>,
) -> Result<ReadFileResult, Error> {
    let target = crate::workspace::resolve(root, path, true)?;
    let cap = max_bytes.unwrap_or(DEFAULT_READ_BYTES).min(MAX_READ_BYTES_HARD);
    let bytes = tokio::fs::read(&target).await.map_err(Error::Io)?;
    let truncated = (bytes.len() as u64) > cap;
    let slice = if truncated {
        &bytes[..cap as usize]
    } else {
        &bytes[..]
    };
    // Decode lossily so binary or mixed files still produce a useful string
    // for the model. Non-UTF8 bytes become U+FFFD.
    let content = String::from_utf8_lossy(slice).into_owned();
    Ok(ReadFileResult {
        path: display_relative(root, &target),
        content,
        truncated,
        bytes_read: slice.len() as u64,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct WriteFileResult {
    pub path: String,
    pub bytes_written: u64,
    pub created: bool,
}

pub async fn write_file(
    root: &Path,
    path: &str,
    content: &str,
) -> Result<WriteFileResult, Error> {
    let target = crate::workspace::resolve(root, path, false)?;
    let created = !target.exists();
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(Error::Io)?;
    }
    tokio::fs::write(&target, content).await.map_err(Error::Io)?;
    Ok(WriteFileResult {
        path: display_relative(root, &target),
        bytes_written: content.len() as u64,
        created,
    })
}

pub async fn append_file(
    root: &Path,
    path: &str,
    content: &str,
) -> Result<WriteFileResult, Error> {
    use tokio::io::AsyncWriteExt;
    let target = crate::workspace::resolve(root, path, false)?;
    let created = !target.exists();
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(Error::Io)?;
    }
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&target)
        .await
        .map_err(Error::Io)?;
    f.write_all(content.as_bytes()).await.map_err(Error::Io)?;
    f.flush().await.map_err(Error::Io)?;
    Ok(WriteFileResult {
        path: display_relative(root, &target),
        bytes_written: content.len() as u64,
        created,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteResult {
    pub path: String,
    pub was_dir: bool,
}

pub async fn delete_path(root: &Path, path: &str) -> Result<DeleteResult, Error> {
    let target = crate::workspace::resolve(root, path, true)?;
    // Don't allow deleting the workspace root itself.
    let root_canon = root.canonicalize().map_err(Error::Io)?;
    if target == root_canon {
        return Err(Error::ToolBadPath(
            "refusing to delete the workspace root".into(),
        ));
    }
    let md = tokio::fs::metadata(&target).await.map_err(Error::Io)?;
    if md.is_dir() {
        // Only empty dirs — we don't recursively remove without an
        // explicit recursive tool, which we haven't shipped.
        tokio::fs::remove_dir(&target).await.map_err(Error::Io)?;
        Ok(DeleteResult {
            path: display_relative(root, &target),
            was_dir: true,
        })
    } else {
        tokio::fs::remove_file(&target).await.map_err(Error::Io)?;
        Ok(DeleteResult {
            path: display_relative(root, &target),
            was_dir: false,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MoveResult {
    pub from: String,
    pub to: String,
}

pub async fn move_path(root: &Path, from: &str, to: &str) -> Result<MoveResult, Error> {
    let from_resolved = crate::workspace::resolve(root, from, true)?;
    let to_resolved = crate::workspace::resolve(root, to, false)?;
    if let Some(parent) = to_resolved.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(Error::Io)?;
    }
    tokio::fs::rename(&from_resolved, &to_resolved)
        .await
        .map_err(Error::Io)?;
    Ok(MoveResult {
        from: display_relative(root, &from_resolved),
        to: display_relative(root, &to_resolved),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateDirResult {
    pub path: String,
}

pub async fn create_dir(root: &Path, path: &str) -> Result<CreateDirResult, Error> {
    let target = crate::workspace::resolve(root, path, false)?;
    tokio::fs::create_dir_all(&target).await.map_err(Error::Io)?;
    Ok(CreateDirResult {
        path: display_relative(root, &target),
    })
}
