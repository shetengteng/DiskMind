use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::db::{Db, TrashItem};

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

/// 把 trash 业务失败追加到 `<app_data>/logs/crash.log`(JSONL),让用户能
/// 离线诊断 Windows 移动失败的具体 `os error` 码。crash_log 写入是 lock-free
/// + 容错,失败不会阻塞调用方,本身就是用来打"应用不致命但用户需要看到"
/// 的事件。
///
/// `stage` 用 `<func>:<step>` 形式(如 `move_to_sandbox:move_file`),
/// 方便在日志面板里筛选;`target` 通常是原始路径或 trash item id;`err`
/// 是原始错误 `to_string()`(已包含 `(os error N)` 后缀)。
fn log_trash_failure(stage: &str, target: &str, err: &str) {
    let _ = crate::crash_log::append(
        "error",
        "trash",
        &format!("{stage}: {target}"),
        err,
    );
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

/// 删除文件,Windows 下对 readonly 属性做一次清理重试。
///
/// 行为约束:
/// 1. 首次 `remove_file` 成功 → 直接返回 Ok。
/// 2. 首次失败 → **保留原始错误**(包括 `raw_os_error`,前端 toast 能看到
///    `(os error 5/32/...)` 区分被占用 vs 无权限 vs readonly)。
/// 3. 仅当 Windows + 元数据显示 readonly 时,清属性后再试一次;清属性
///    或第二次 remove 失败,**仍返回最初的 first_err**,不要被中间错误
///    覆盖根本原因。
/// 4. 非 Windows / 非 readonly → 直接 propagate first_err。
pub fn force_remove(p: &Path) -> std::io::Result<()> {
    let first_err = match std::fs::remove_file(p) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    #[cfg(windows)]
    {
        if let Ok(meta) = std::fs::metadata(p) {
            let mut perms = meta.permissions();
            if perms.readonly() {
                perms.set_readonly(false);
                if std::fs::set_permissions(p, perms).is_ok() {
                    if std::fs::remove_file(p).is_ok() {
                        return Ok(());
                    }
                }
            }
        }
    }

    Err(first_err)
}

/// 把文件从 `src` 移动到 `dst`,失败时**绝不留垃圾**。
///
/// 策略:
/// 1. **同卷 fast path**:`fs::rename` 原子完成。
/// 2. **rename 失败但不是跨卷错** → 立即返回错误,不掩盖根本原因
///    (Windows 上的 "被占用"/"无权限"/"路径过长" 一定要原样上报,否则
///    走 copy 也只会再失败一次,用户看到的是 copy 错误,无法诊断)。
/// 3. **Windows readonly src** → rename/copy 都可能失败,尝试清掉 readonly
///    后重试 rename 一次。
/// 4. **跨卷场景** (`ERROR_NOT_SAME_DEVICE=17` / `EXDEV=18`) → copy + remove。
///    - copy 失败 → 清掉 dst 不完整副本。
///    - remove src 失败 → **回滚 dst**,保证 src 完整不丢数据,把 remove
///      的错误透传给用户。
fn move_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    let rename_err = match std::fs::rename(src, dst) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    #[cfg(windows)]
    if rename_err.kind() == std::io::ErrorKind::PermissionDenied
        && try_clear_readonly(src).is_ok()
    {
        if std::fs::rename(src, dst).is_ok() {
            return Ok(());
        }
    }

    if !is_cross_device_error(&rename_err) {
        return Err(rename_err);
    }

    #[cfg(windows)]
    let _ = try_clear_readonly(src);

    if let Err(e) = std::fs::copy(src, dst) {
        let _ = std::fs::remove_file(dst);
        return Err(e);
    }

    if let Err(e) = force_remove(src) {
        let _ = std::fs::remove_file(dst);
        return Err(e);
    }

    Ok(())
}

/// 跨卷错误码识别。Windows `ERROR_NOT_SAME_DEVICE` = 17,Unix `EXDEV` = 18。
/// 标准库 1.85+ 有 `ErrorKind::CrossesDevices` 但本项目 MSRV 较老,直接用
/// `raw_os_error` 兜底。
fn is_cross_device_error(e: &std::io::Error) -> bool {
    #[cfg(windows)]
    return e.raw_os_error() == Some(17);
    #[cfg(unix)]
    return e.raw_os_error() == Some(18);
    #[cfg(not(any(windows, unix)))]
    return false;
}

/// Windows 专用:清除文件的 readonly 属性。源文件本身没 readonly 的话
/// 是 no-op。返回 metadata 读取 / set_permissions 调用的原始错误,调用方
/// 可以决定是否容忍。
#[cfg(windows)]
fn try_clear_readonly(p: &Path) -> std::io::Result<()> {
    let meta = std::fs::metadata(p)?;
    let mut perms = meta.permissions();
    if perms.readonly() {
        perms.set_readonly(false);
        std::fs::set_permissions(p, perms)?;
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
            log_trash_failure("move_to_sandbox:source_missing", &req.path, "src is not a file");
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
                let err_str = e.to_string();
                log_trash_failure("move_to_sandbox:db_insert", &req.path, &err_str);
                failures.push(TrashFailure {
                    path: req.path,
                    message: crate::i18n::i18n_p(
                        "trash.error.db_insert_failed",
                        &[("err", &err_str)],
                    ),
                });
                continue;
            }
        };

        let dst = sandbox_root.join(sandbox_filename(id, &src));
        if let Err(e) = move_file(&src, &dst) {
            let err_str = e.to_string();
            log_trash_failure(
                "move_to_sandbox:move_file",
                &format!("{} -> {}", req.path, dst.display()),
                &err_str,
            );
            // 1) 回滚刚才占位插入的行
            let _ = db.trash_mark_deleted(id, ts);
            // 2) `move_file` 内部已经做过 dst 回滚,这里再兜底一次 —
            //    防止未来某条新失败路径漏掉清理,造成沙箱孤儿副本(用户
            //    回收站列表是空的但磁盘里有,30 天后才被 cleanup 兜走)
            if dst.exists() {
                let _ = std::fs::remove_file(&dst);
            }
            failures.push(TrashFailure {
                path: req.path,
                message: crate::i18n::i18n_p(
                    "trash.error.move_failed",
                    &[("err", &err_str)],
                ),
            });
            continue;
        }

        // 用真实路径覆盖 sandbox_path
        if let Err(e) = db.trash_set_sandbox_path(id, &dst.to_string_lossy()) {
            let err_str = e.to_string();
            log_trash_failure("move_to_sandbox:update_path", &req.path, &err_str);
            failures.push(TrashFailure {
                path: req.path,
                message: crate::i18n::i18n_p(
                    "trash.error.update_sandbox_failed",
                    &[("err", &err_str)],
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
                log_trash_failure(
                    "restore_items:item_missing",
                    &format!("#{}", id),
                    "row not in_trash or not found",
                );
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: crate::i18n::i18n("trash.error.item_missing"),
                });
                continue;
            }
            Err(e) => {
                let err_str = e.to_string();
                log_trash_failure("restore_items:query_failed", &format!("#{}", id), &err_str);
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: crate::i18n::i18n_p(
                        "trash.error.query_failed",
                        &[("err", &err_str)],
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
            log_trash_failure(
                "restore_items:target_exists",
                &item.original_path,
                "destination already exists",
            );
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: crate::i18n::i18n("trash.error.restore_target_exists"),
            });
            continue;
        }

        if let Err(e) = move_file(&src, &dst) {
            let err_str = e.to_string();
            log_trash_failure(
                "restore_items:move_file",
                &format!("{} -> {}", src.display(), item.original_path),
                &err_str,
            );
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: crate::i18n::i18n_p(
                    "trash.error.restore_failed",
                    &[("err", &err_str)],
                ),
            });
            continue;
        }

        if let Err(e) = db.trash_mark_restored(id, ts) {
            let err_str = e.to_string();
            log_trash_failure("restore_items:mark_restored", &item.original_path, &err_str);
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: crate::i18n::i18n_p(
                    "trash.error.mark_restored_failed",
                    &[("err", &err_str)],
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
                log_trash_failure(
                    "delete_items:item_missing",
                    &format!("#{}", id),
                    "row not in_trash or not found",
                );
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: crate::i18n::i18n("trash.error.item_missing"),
                });
                continue;
            }
            Err(e) => {
                let err_str = e.to_string();
                log_trash_failure("delete_items:query_failed", &format!("#{}", id), &err_str);
                failures.push(TrashFailure {
                    path: format!("#{}", id),
                    message: crate::i18n::i18n_p(
                        "trash.error.query_failed",
                        &[("err", &err_str)],
                    ),
                });
                continue;
            }
        };

        let sandbox = PathBuf::from(&item.sandbox_path);
        if sandbox.exists() {
            if let Err(e) = force_remove(&sandbox) {
                let err_str = e.to_string();
                log_trash_failure(
                    "delete_items:force_remove",
                    &item.sandbox_path,
                    &err_str,
                );
                failures.push(TrashFailure {
                    path: item.original_path.clone(),
                    message: crate::i18n::i18n_p(
                        "trash.error.physical_delete_failed",
                        &[("err", &err_str)],
                    ),
                });
                continue;
            }
        }

        if let Err(e) = db.trash_mark_deleted(id, ts) {
            let err_str = e.to_string();
            log_trash_failure("delete_items:mark_deleted", &item.original_path, &err_str);
            failures.push(TrashFailure {
                path: item.original_path.clone(),
                message: crate::i18n::i18n_p(
                    "trash.error.mark_deleted_failed",
                    &[("err", &err_str)],
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
            let err_str = e.to_string();
            log_trash_failure("empty_all:list", "—", &err_str);
            return TrashMoveResult {
                items: Vec::new(),
                failures: vec![TrashFailure {
                    path: "—".into(),
                    message: crate::i18n::i18n_p(
                        "trash.error.query_sandbox_failed",
                        &[("err", &err_str)],
                    ),
                }],
            }
        }
    };
    let ids: Vec<i64> = list.into_iter().map(|i| i.id).collect();
    delete_items(db, ids)
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
