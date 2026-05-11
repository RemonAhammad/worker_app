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

// ---------------------------------------------------------------------------
// Search.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub line_number: u32,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
    pub files_scanned: u32,
}

const SEARCH_MAX_CEILING: u32 = 1000;
const SEARCH_DEFAULT_CAP: u32 = 100;
const SEARCH_MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Plain-substring search across files under `start`. Skips hidden
/// directories (`.git`, `.cache`, etc.), binary-looking files, and any
/// file larger than `SEARCH_MAX_FILE_BYTES`. Result count is capped.
pub async fn search(
    root: &Path,
    query: &str,
    path: Option<&str>,
    max_results: Option<u32>,
    case_insensitive: bool,
) -> Result<SearchResult, Error> {
    if query.is_empty() {
        return Err(Error::BadArgs("query must not be empty".into()));
    }
    let start = match path {
        Some(p) => crate::workspace::resolve(root, p, true)?,
        None => root.canonicalize().map_err(Error::Io)?,
    };
    let cap = max_results.unwrap_or(SEARCH_DEFAULT_CAP).min(SEARCH_MAX_CEILING) as usize;
    let needle_lower = query.to_lowercase();

    // walk_dir + read each file. We do it on a blocking task so we can use
    // std::fs without holding the async runtime hostage on a large tree.
    let root_canon = root
        .canonicalize()
        .map_err(Error::Io)?;
    let query_owned = query.to_string();
    let needle_lower_owned = needle_lower.clone();
    let task = tokio::task::spawn_blocking(move || -> Result<SearchResult, Error> {
        let mut matches: Vec<SearchMatch> = Vec::new();
        let mut files_scanned = 0u32;
        let mut truncated = false;
        let mut stack: Vec<std::path::PathBuf> = vec![start];

        while let Some(dir) = stack.pop() {
            let read = match std::fs::read_dir(&dir) {
                Ok(r) => r,
                Err(_) => continue,
            };
            for entry in read.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip hidden / heavy / vendor dirs.
                if name_str.starts_with('.')
                    || name_str == "node_modules"
                    || name_str == "target"
                    || name_str == "dist"
                    || name_str == "dist-js"
                {
                    continue;
                }
                let ft = match entry.file_type() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let path = entry.path();
                if ft.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !ft.is_file() {
                    continue;
                }
                let md = match path.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if md.len() > SEARCH_MAX_FILE_BYTES {
                    continue;
                }
                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                // Cheap binary check: if the first chunk has a NUL byte,
                // treat as binary and skip.
                let probe_end = bytes.len().min(8192);
                if bytes[..probe_end].contains(&0u8) {
                    continue;
                }
                let text = String::from_utf8_lossy(&bytes);
                files_scanned += 1;
                for (idx, line) in text.lines().enumerate() {
                    let hit = if case_insensitive {
                        line.to_lowercase().contains(&needle_lower_owned)
                    } else {
                        line.contains(&query_owned)
                    };
                    if !hit {
                        continue;
                    }
                    if matches.len() >= cap {
                        truncated = true;
                        break;
                    }
                    matches.push(SearchMatch {
                        path: path
                            .strip_prefix(&root_canon)
                            .ok()
                            .and_then(|r| r.to_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| path.display().to_string()),
                        line_number: (idx + 1) as u32,
                        // Truncate very long lines so tool output stays sane.
                        line: if line.len() > 400 {
                            format!("{}…", &line[..400])
                        } else {
                            line.to_string()
                        },
                    });
                }
                if matches.len() >= cap {
                    truncated = true;
                    break;
                }
            }
            if matches.len() >= cap {
                truncated = true;
                break;
            }
        }

        Ok(SearchResult {
            query: query_owned,
            matches,
            truncated,
            files_scanned,
        })
    });
    task.await.map_err(|e| Error::BadArgs(format!("search join error: {e}")))?
}

// ---------------------------------------------------------------------------
// Diff preview for write_file approval cards.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct WritePreview {
    pub path: String,
    pub exists: bool,
    /// Up to 256 lines of diff hunks rendered with line markers (`-`/`+`/` `).
    pub diff: String,
    pub old_bytes: u64,
    pub new_bytes: u64,
}

pub async fn preview_write(
    root: &Path,
    path: &str,
    new_content: &str,
) -> Result<WritePreview, Error> {
    let target = crate::workspace::resolve(root, path, false)?;
    let exists = target.exists();
    let old = if exists {
        tokio::fs::read_to_string(&target).await.unwrap_or_default()
    } else {
        String::new()
    };
    let diff = render_unified_diff(&old, new_content);
    Ok(WritePreview {
        path: display_relative(root, &target),
        exists,
        diff,
        old_bytes: old.len() as u64,
        new_bytes: new_content.len() as u64,
    })
}

/// Minimal unified-diff renderer. Uses a longest-common-subsequence based
/// pass that is OK for the small/medium files the model will typically
/// write; we cap the rendered output so an aggressive change doesn't
/// flood the UI.
fn render_unified_diff(a: &str, b: &str) -> String {
    const MAX_LINES: usize = 256;
    let a_lines: Vec<&str> = a.lines().collect();
    let b_lines: Vec<&str> = b.lines().collect();

    // O(n*m) LCS table. For files up to ~2k lines this is fine; beyond that
    // we fall back to a "summary" header since the table allocation is large.
    if a_lines.len() * b_lines.len() > 2_000_000 {
        return format!(
            "(diff too large to render: {} → {} lines)",
            a_lines.len(),
            b_lines.len()
        );
    }

    let n = a_lines.len();
    let m = b_lines.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            dp[i + 1][j + 1] = if a_lines[i] == b_lines[j] {
                dp[i][j] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    // Backtrack to produce edits, then reverse.
    let mut edits: Vec<(char, String)> = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a_lines[i - 1] == b_lines[j - 1] {
            edits.push((' ', a_lines[i - 1].to_string()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            edits.push(('+', b_lines[j - 1].to_string()));
            j -= 1;
        } else {
            edits.push(('-', a_lines[i - 1].to_string()));
            i -= 1;
        }
    }
    edits.reverse();

    let mut out = String::new();
    for (k, (sign, line)) in edits.iter().enumerate() {
        if k >= MAX_LINES {
            out.push_str(&format!(
                "… {} more change line(s) hidden\n",
                edits.len() - MAX_LINES
            ));
            break;
        }
        out.push(*sign);
        out.push(' ');
        // Avoid runaway lines.
        if line.len() > 400 {
            out.push_str(&line[..400]);
            out.push('…');
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str("(no textual changes)\n");
    }
    out
}
