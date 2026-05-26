use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::db::{Db, TrashItem, TrashStats};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashMoveRequest {
    pub path: String,
    pub size_bytes: u64,
    pub category: String,
    pub risk: String,
    pub ai_reason: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashMoveResult {
    pub items: Vec<TrashItem>,
    pub failures: Vec<TrashFailure>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashFailure {
    pub path: String,
    pub message: String,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn ensure_dir(p: &Path) -> std::io::Result<()> {
    if !p.exists() {
        std::fs::create_dir_all(p)?;
    }
    Ok(())
}

fn sandbox_filename(id: i64, original: &Path) -> String {
    let leaf = original
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    format!("{}__{}", id, leaf)
}

fn move_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Err(_) = std::fs::rename(src, dst) {
        // cross-volume / permission fallback
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
    }
    Ok(())
}

pub fn move_to_sandbox(
    db: &Arc<Db>,
    sandbox_root: &Path,
    requests: Vec<TrashMoveRequest>,
) -> TrashMoveResult {
    let _ = ensure_dir(sandbox_root);
    let mut items = Vec::with_capacity(requests.len());
    let mut failures = Vec::new();
    let ts = now_ms();

    for req in requests {
        let src = PathBuf::from(&req.path);
        if !src.is_file() {
            failures.push(TrashFailure {
                path: req.path,
                message: "源文件不存在".into(),
            });
            continue;
        }

        // pre-insert with placeholder sandbox_path to obtain id
        let placeholder = sandbox_root.join("__pending__");
        let id = match db.trash_insert(
            &req.path,
            &placeholder.to_string_lossy(),
            req.size_bytes,
            &req.category,
            &req.risk,
            &req.ai_reason,
            ts,
        ) {
            Ok(id) => id,
            Err(e) => {
                failures.push(TrashFailure {
                    path: req.path,
                    message: format!("写入数据库失败: {}", e),
                });
                continue;
            }
        };

        let dst = sandbox_root.join(sandbox_filename(id, &src));
        if let Err(e) = move_file(&src, &dst) {
            // rollback row
            let _ = db.trash_mark_deleted(id, ts);
            failures.push(TrashFailure {
                path: req.path,
                message: format!("移动失败: {}", e),
            });
            continue;
        }

        // update sandbox_path with real one
        if let Err(e) = db.trash_set_sandbox_path(id, &dst.to_string_lossy()) {
            failures.push(TrashFailure {
                path: req.path,
                message: format!("更新沙箱路径失败: {}", e),
            });
            continue;
        }

        if let Ok(Some(item)) = db.trash_get(id) {
            items.push(item);
        }
    }

    TrashMoveResult { items, failures }
}

pub fn restore_items(db: &Arc<Db>, ids: Vec<i64>) -> TrashMoveResult {
    let mut items = Vec::new();
    let mut failures = Vec::new();
    let ts = now_ms();

    for id in ids {
        let item = match db.trash_get(id) {
            Ok(Some(it)) if it.status == "in_trash" => it,
            Ok(_) => {
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: "项目不存在或已处理".into(),
                });
                continue;
            }
            Err(e) => {
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: format!("查询失败: {}", e),
                });
                continue;
            }
        };

        let src = PathBuf::from(&item.sandbox_path);
        let dst = PathBuf::from(&item.original_path);
        if let Some(parent) = dst.parent() {
            let _ = ensure_dir(parent);
        }

        if dst.exists() {
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: "原位置已存在同名文件,跳过恢复".into(),
            });
            continue;
        }

        if let Err(e) = move_file(&src, &dst) {
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: format!("恢复失败: {}", e),
            });
            continue;
        }

        if let Err(e) = db.trash_mark_restored(id, ts) {
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: format!("标记恢复失败: {}", e),
            });
            continue;
        }

        if let Ok(Some(it)) = db.trash_get(id) {
            items.push(it);
        }
    }

    TrashMoveResult { items, failures }
}

pub fn delete_items(db: &Arc<Db>, ids: Vec<i64>) -> TrashMoveResult {
    let mut items = Vec::new();
    let mut failures = Vec::new();
    let ts = now_ms();

    for id in ids {
        let item = match db.trash_get(id) {
            Ok(Some(it)) if it.status == "in_trash" => it,
            Ok(_) => {
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: "项目不存在或已处理".into(),
                });
                continue;
            }
            Err(e) => {
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: format!("查询失败: {}", e),
                });
                continue;
            }
        };

        let sandbox = PathBuf::from(&item.sandbox_path);
        if sandbox.exists() {
            if let Err(e) = std::fs::remove_file(&sandbox) {
                failures.push(TrashFailure {
                    path: item.original_path.clone(),
                    message: format!("物理删除失败: {}", e),
                });
                continue;
            }
        }

        if let Err(e) = db.trash_mark_deleted(id, ts) {
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: format!("标记删除失败: {}", e),
            });
            continue;
        }

        if let Ok(Some(it)) = db.trash_get(id) {
            items.push(it);
        }
    }

    TrashMoveResult { items, failures }
}

pub fn empty_all(db: &Arc<Db>) -> TrashMoveResult {
    let list = match db.trash_list() {
        Ok(l) => l,
        Err(e) => {
            return TrashMoveResult {
                items: Vec::new(),
                failures: vec![TrashFailure {
                    path: "—".into(),
                    message: format!("查询沙箱失败: {}", e),
                }],
            }
        }
    };
    let ids: Vec<i64> = list.into_iter().map(|i| i.id).collect();
    delete_items(db, ids)
}

pub fn stats(db: &Arc<Db>) -> rusqlite::Result<TrashStats> {
    db.trash_stats()
}
