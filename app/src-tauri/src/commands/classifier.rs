//! Round 29 · classifier 用户规则的运行时管理 IPC。
//!
//! 提供两个命令:
//! - `classifier_reload_user_rules` — 用户改完 `app_data/rules.toml` 后调一次
//!   即可生效,不需要重启应用。前端 Settings 页可以放一个"刷新规则"按钮。
//! - `classifier_user_rules_path` — 返回 rules.toml 的绝对路径,UI 可以
//!   提供"在 Finder 中显示"或"在编辑器中打开"快捷入口让用户找到这个文件。

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::classifier::user_rules;

/// rules.toml 在 app_data_dir 里的固定相对路径。
const USER_RULES_FILE: &str = "rules.toml";

fn rules_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?;
    Ok(dir.join(USER_RULES_FILE))
}

/// 重新加载 `app_data/rules.toml`。返回当前 ruleset 里的规则数(让前端
/// 能显示 "已加载 N 条用户规则")。文件不存在或解析失败返回 0,与 setup
/// 阶段的容错策略一致 — 不让一个坏 TOML 阻断 reload 调用,内部细节通过
/// 后端日志暴露。
#[tauri::command]
pub fn classifier_reload_user_rules(app: AppHandle) -> Result<usize, String> {
    let path = rules_path(&app)?;
    let set = user_rules::load_from(&path);
    let count = set.rule.len();
    user_rules::install(set);
    Ok(count)
}

/// 拿用户规则文件路径供 UI 展示 / "Reveal in Finder"。即使文件不存在
/// 也返回路径(让前端能 `revealInExplorer` 父目录或弹窗提示创建)。
#[tauri::command]
pub fn classifier_user_rules_path(app: AppHandle) -> Result<String, String> {
    let path = rules_path(&app)?;
    Ok(path.to_string_lossy().to_string())
}
