//! Round 29 / 34B · classifier 用户规则的运行时管理 IPC。
//!
//! 提供 4 个命令:
//! - `classifier_reload_user_rules` — 用户改完 `app_data/rules.toml` 后调一次
//!   即可生效,不需要重启应用。前端 Settings 页可以放一个"刷新规则"按钮。
//! - `classifier_user_rules_path` — 返回 rules.toml 的绝对路径,UI 可以
//!   提供"在 Finder 中显示"或"在编辑器中打开"快捷入口让用户找到这个文件。
//! - `classifier_list_user_rules` (Round 34B) — 给 UI 编辑器读取当前规则集
//!   的结构化版本(`UserRuleSet`),前端可以渲染成 form/table CRUD。
//! - `classifier_save_user_rules` (Round 34B) — UI 编辑器写盘 + 自动 reload。

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::classifier::user_rules::{self, UserRuleSet};

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

/// Round 34B · 读取当前 rules.toml 的结构化内容。
///
/// 与 `classifier_reload_user_rules` 的差别:reload 把规则装进**全局**
/// snapshot 并返回 count;list 只读取磁盘文件本身(**不**触碰全局),
/// 给 UI 编辑器作为可编辑表单的初始值。这样 UI 上的 "未保存" 草稿就
/// 不会污染分类 hot path。
///
/// 文件不存在直接返回空 set,**不报错** — 让 UI 一进来就显示"还没规则
/// 来加一条吧"。
#[tauri::command]
pub fn classifier_list_user_rules(app: AppHandle) -> Result<UserRuleSet, String> {
    let path = rules_path(&app)?;
    Ok(user_rules::load_from(&path))
}

/// Round 34B · UI 编辑器保存按钮:把整套规则写回 rules.toml + 立即 reload。
///
/// 写盘失败立刻报错,**不**改全局 snapshot(用户的本地编辑保留在 UI 里
/// 可以再试);写盘成功后才 install 到全局,确保磁盘 / 内存一致。
///
/// 返回写盘后的规则数,前端 toast 可以显示 "已保存 N 条规则"。
#[tauri::command]
pub fn classifier_save_user_rules(
    app: AppHandle,
    rules: UserRuleSet,
) -> Result<usize, String> {
    let path = rules_path(&app)?;
    user_rules::save_to(&path, &rules)?;
    let count = rules.rule.len();
    user_rules::install(rules);
    Ok(count)
}
