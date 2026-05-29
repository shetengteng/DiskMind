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
        // 跨卷 / 权限失败时回退为 copy + remove
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
                message: crate::i18n::i18n("trash.error.source_missing"),
            });
            continue;
        }

        // 先用占位 sandbox_path 插入,以获得自增 id
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
                    message: crate::i18n::i18n_p(
                        "trash.error.db_insert_failed",
                        &[("err", &e.to_string())],
                    ),
                });
                continue;
            }
        };

        let dst = sandbox_root.join(sandbox_filename(id, &src));
        if let Err(e) = move_file(&src, &dst) {
            // 回滚刚才占位插入的行
            let _ = db.trash_mark_deleted(id, ts);
            failures.push(TrashFailure {
                path: req.path,
                message: crate::i18n::i18n_p(
                    "trash.error.move_failed",
                    &[("err", &e.to_string())],
                ),
            });
            continue;
        }

        // 用真实路径覆盖 sandbox_path
        if let Err(e) = db.trash_set_sandbox_path(id, &dst.to_string_lossy()) {
            failures.push(TrashFailure {
                path: req.path,
                message: crate::i18n::i18n_p(
                    "trash.error.update_sandbox_failed",
                    &[("err", &e.to_string())],
                ),
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
                    message: crate::i18n::i18n("trash.error.item_missing"),
                });
                continue;
            }
            Err(e) => {
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: crate::i18n::i18n_p(
                        "trash.error.query_failed",
                        &[("err", &e.to_string())],
                    ),
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
                message: crate::i18n::i18n("trash.error.restore_target_exists"),
            });
            continue;
        }

        if let Err(e) = move_file(&src, &dst) {
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: crate::i18n::i18n_p(
                    "trash.error.restore_failed",
                    &[("err", &e.to_string())],
                ),
            });
            continue;
        }

        if let Err(e) = db.trash_mark_restored(id, ts) {
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: crate::i18n::i18n_p(
                    "trash.error.mark_restored_failed",
                    &[("err", &e.to_string())],
                ),
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
                    message: crate::i18n::i18n("trash.error.item_missing"),
                });
                continue;
            }
            Err(e) => {
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: crate::i18n::i18n_p(
                        "trash.error.query_failed",
                        &[("err", &e.to_string())],
                    ),
                });
                continue;
            }
        };

        let sandbox = PathBuf::from(&item.sandbox_path);
        if sandbox.exists() {
            if let Err(e) = std::fs::remove_file(&sandbox) {
                failures.push(TrashFailure {
                    path: item.original_path.clone(),
                    message: crate::i18n::i18n_p(
                        "trash.error.physical_delete_failed",
                        &[("err", &e.to_string())],
                    ),
                });
                continue;
            }
        }

        if let Err(e) = db.trash_mark_deleted(id, ts) {
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: crate::i18n::i18n_p(
                    "trash.error.mark_deleted_failed",
                    &[("err", &e.to_string())],
                ),
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
                    message: crate::i18n::i18n_p(
                        "trash.error.query_sandbox_failed",
                        &[("err", &e.to_string())],
                    ),
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

/// 删除沙箱中早于 `retention_days` 的项目。返回成功清理的条目数。
/// 由 `lib.rs::run` 中的后台任务周期性调用,并在应用启动时执行一次。
pub fn cleanup_expired(db: &Arc<Db>, retention_days: u64) -> u64 {
    let cutoff = now_ms() - (retention_days as i64) * 24 * 3600 * 1000;
    let stale = match db.trash_list_stale(cutoff) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[diskmind] trash_list_stale failed: {e}");
            return 0;
        }
    };
    if stale.is_empty() {
        return 0;
    }
    let ids: Vec<i64> = stale.iter().map(|i| i.id).collect();
    let result = delete_items(db, ids);
    let purged = result.items.len() as u64;
    if !result.failures.is_empty() {
        for f in &result.failures {
            eprintln!("[diskmind] cleanup failure for {}: {}", f.path, f.message);
        }
    }
    purged
}
