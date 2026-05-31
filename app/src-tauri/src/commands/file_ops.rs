use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::file_search::is_protected;
use crate::db::Db;
use crate::state::{now_ms, ScanState};
use crate::trash;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum FileOpsRequest {
    Move {
        paths: Vec<String>,
        destination: String,
    },
    Rename {
        items: Vec<RenameItem>,
    },
    Delete {
        paths: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameItem {
    pub path: String,
    pub new_name: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileOpsSuccess {
    pub source_path: String,
    pub dest_path: Option<String>,
    pub op_type: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileOpsFailure {
    pub source_path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOpsResult {
    pub succeeded: Vec<FileOpsSuccess>,
    pub failures: Vec<FileOpsFailure>,
}

fn move_file_or_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        if let Err(_) = std::fs::rename(src, dst) {
            copy_dir_recursive(src, dst)?;
            std::fs::remove_dir_all(src)?;
        }
    } else {
        if let Err(_) = std::fs::rename(src, dst) {
            std::fs::copy(src, dst)?;
            crate::trash::force_remove(src)?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn log_op(db: &Arc<Db>, op_type: &str, source: &str, dest: Option<&str>, size: Option<u64>, status: &str, error: Option<&str>, ai_query: Option<&str>) {
    let _ = db.file_ops_log_insert(op_type, source, dest, size, status, error, ai_query, now_ms());
}

fn exec_move(db: &Arc<Db>, paths: Vec<String>, destination: String) -> FileOpsResult {
    let mut succeeded = Vec::new();
    let mut failures = Vec::new();
    let dest_dir = PathBuf::from(&destination);

    if !dest_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&dest_dir) {
            return FileOpsResult {
                succeeded: Vec::new(),
                failures: vec![FileOpsFailure {
                    source_path: destination,
                    message: format!("Cannot create destination: {e}"),
                }],
            };
        }
    }

    for src_str in paths {
        if is_protected(&src_str) {
            failures.push(FileOpsFailure {
                source_path: src_str.clone(),
                message: "Protected system path".to_string(),
            });
            log_op(db, "move", &src_str, Some(&destination), None, "failed", Some("protected path"), None);
            continue;
        }

        let src = PathBuf::from(&src_str);
        if !src.exists() {
            failures.push(FileOpsFailure {
                source_path: src_str.clone(),
                message: "Source does not exist".to_string(),
            });
            log_op(db, "move", &src_str, Some(&destination), None, "failed", Some("not found"), None);
            continue;
        }

        let file_name = src.file_name().unwrap_or_default();
        let dst = dest_dir.join(file_name);

        if dst.exists() {
            failures.push(FileOpsFailure {
                source_path: src_str.clone(),
                message: format!("Target already exists: {}", dst.display()),
            });
            log_op(db, "move", &src_str, Some(&dst.to_string_lossy()), None, "failed", Some("target exists"), None);
            continue;
        }

        let size = src.metadata().ok().map(|m| m.len());
        match move_file_or_dir(&src, &dst) {
            Ok(()) => {
                let dest_str = dst.to_string_lossy().to_string();
                log_op(db, "move", &src_str, Some(&dest_str), size, "ok", None, None);
                succeeded.push(FileOpsSuccess {
                    source_path: src_str,
                    dest_path: Some(dest_str),
                    op_type: "move".to_string(),
                });
            }
            Err(e) => {
                log_op(db, "move", &src_str, Some(&destination), size, "failed", Some(&e.to_string()), None);
                failures.push(FileOpsFailure {
                    source_path: src_str,
                    message: e.to_string(),
                });
            }
        }
    }

    FileOpsResult { succeeded, failures }
}

fn exec_rename(db: &Arc<Db>, items: Vec<RenameItem>) -> FileOpsResult {
    let mut succeeded = Vec::new();
    let mut failures = Vec::new();

    for item in items {
        if is_protected(&item.path) {
            failures.push(FileOpsFailure {
                source_path: item.path.clone(),
                message: "Protected system path".to_string(),
            });
            continue;
        }

        let src = PathBuf::from(&item.path);
        if !src.exists() {
            failures.push(FileOpsFailure {
                source_path: item.path.clone(),
                message: "Source does not exist".to_string(),
            });
            continue;
        }

        let parent = match src.parent() {
            Some(p) => p,
            None => {
                failures.push(FileOpsFailure {
                    source_path: item.path.clone(),
                    message: "Cannot determine parent directory".to_string(),
                });
                continue;
            }
        };

        let dst = parent.join(&item.new_name);
        if dst.exists() {
            failures.push(FileOpsFailure {
                source_path: item.path.clone(),
                message: format!("Target already exists: {}", dst.display()),
            });
            continue;
        }

        match std::fs::rename(&src, &dst) {
            Ok(()) => {
                let dest_str = dst.to_string_lossy().to_string();
                log_op(db, "rename", &item.path, Some(&dest_str), None, "ok", None, None);
                succeeded.push(FileOpsSuccess {
                    source_path: item.path,
                    dest_path: Some(dest_str),
                    op_type: "rename".to_string(),
                });
            }
            Err(e) => {
                log_op(db, "rename", &item.path, None, None, "failed", Some(&e.to_string()), None);
                failures.push(FileOpsFailure {
                    source_path: item.path,
                    message: e.to_string(),
                });
            }
        }
    }

    FileOpsResult { succeeded, failures }
}

fn exec_delete(db: &Arc<Db>, sandbox_root: &Path, paths: Vec<String>) -> FileOpsResult {
    let reqs: Vec<trash::TrashMoveRequest> = paths
        .iter()
        .filter_map(|p| {
            let pb = PathBuf::from(p);
            let size = pb.metadata().ok().map(|m| m.len()).unwrap_or(0);
            Some(trash::TrashMoveRequest {
                path: p.clone(),
                size_bytes: size,
                category: "file_ops_delete".to_string(),
                risk: "low".to_string(),
                ai_reason: "User requested deletion via AI file manager".to_string(),
            })
        })
        .collect();

    let trash_result = trash::move_to_sandbox(db, sandbox_root, reqs);

    for item in &trash_result.items {
        log_op(db, "delete", &item.original_path, None, Some(item.size_bytes as u64), "ok", None, None);
    }
    for fail in &trash_result.failures {
        log_op(db, "delete", &fail.path, None, None, "failed", Some(&fail.message), None);
    }

    FileOpsResult {
        succeeded: trash_result
            .items
            .into_iter()
            .map(|i| FileOpsSuccess {
                source_path: i.original_path,
                dest_path: None,
                op_type: "delete".to_string(),
            })
            .collect(),
        failures: trash_result
            .failures
            .into_iter()
            .map(|f| FileOpsFailure {
                source_path: f.path,
                message: f.message,
            })
            .collect(),
    }
}

#[tauri::command]
pub fn file_ops_execute(
    request: FileOpsRequest,
    state: State<'_, ScanState>,
) -> Result<FileOpsResult, String> {
    let db = &state.db;
    let result = match request {
        FileOpsRequest::Move { paths, destination } => exec_move(db, paths, destination),
        FileOpsRequest::Rename { items } => exec_rename(db, items),
        FileOpsRequest::Delete { paths } => exec_delete(db, &state.sandbox_root, paths),
    };
    Ok(result)
}

#[tauri::command]
pub fn file_ops_history(
    limit: Option<u32>,
    state: State<'_, ScanState>,
) -> Result<Vec<crate::db::FileOpsLogEntry>, String> {
    state
        .db
        .file_ops_log_list(limit.unwrap_or(100) as i64)
        .map_err(|e| e.to_string())
}
