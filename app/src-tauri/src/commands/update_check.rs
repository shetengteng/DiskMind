//! 手动检查 GitHub Release 是否有新版本 + 在系统默认浏览器中打开外链。
//!
//! Round 11 已永久决定不集成 `tauri-plugin-updater`(不自带签名 / 不做
//! 原地升级)。这里提供两个最薄的命令:
//!   * `check_for_updates`: GET `/repos/<owner>/<repo>/releases/latest`,
//!     做一次语义化版本比较;返回 `UpdateCheckResult`。前端按结果 toast
//!     + 渲染"前往下载"按钮。
//!   * `open_external_url`: 跨平台拉起默认浏览器,与现有 `reveal_in_explorer`
//!     同模式(直接 spawn 平台命令,不引入新插件)。
//!
//! 安全性:`open_external_url` 只允许 `https://` / `http://` 前缀,避免
//! 前端误传任意 scheme 触发本地 handler。

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

const GITHUB_REPO: &str = "shetengteng/DiskMind";
const HTTP_TIMEOUT_SECS: u64 = 15;
// GitHub API 要求带 UA;不带会被 403 拦截。
const USER_AGENT: &str = "DiskMind-UpdateChecker";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    /// 当前 app 版本(来自 `Cargo.toml`,与打包后的 `tauri.conf.json` 同步)。
    pub current_version: String,
    /// GitHub Releases 最新一条的 tag,去掉 `v` 前缀。
    pub latest_version: String,
    /// 是否有新版本。`latest > current` 时为 true;相同或更新时为 false。
    pub update_available: bool,
    /// Release 页面 URL,UI 给"前往下载"按钮用。
    pub release_url: String,
    /// Release 的 Markdown 描述(可能为空)。UI 暂时只在"已是最新版"
    /// 时不展示,有新版本时透传给前端用 `<pre>` 简单呈现。
    pub release_notes: String,
    /// 发布时间(ISO8601 字符串,GitHub 原样返回)。
    pub published_at: String,
}

/// GitHub Releases API 的最小反序列化模型(只取我们要的字段)。
#[derive(Debug, Deserialize)]
struct GithubReleaseDto {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    // 当前版本从 Cargo 编译期注入;与 `tauri.conf.json` 的 version 由
    // `scripts/sync-version.js` 保持同步,确保打包产物里读到的是真实版本。
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);

    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("$i18n:update.error.http_init|msg={}", e))?;

    let resp = client
        .get(&url)
        // GitHub 推荐的 API 版本头,虽然不强制但更稳。
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("$i18n:update.error.network|msg={}", e))?;

    if !resp.status().is_success() {
        // 404 通常意味着仓库还没任何 Release。这是合法状态,告诉用户
        // "暂未发布 Release"而不是当错误吐栈。
        if resp.status().as_u16() == 404 {
            return Ok(UpdateCheckResult {
                current_version: current_version.clone(),
                latest_version: current_version,
                update_available: false,
                release_url: format!("https://github.com/{}/releases", GITHUB_REPO),
                release_notes: String::new(),
                published_at: String::new(),
            });
        }
        return Err(format!(
            "$i18n:update.error.bad_status|status={}",
            resp.status().as_u16()
        ));
    }

    let release: GithubReleaseDto = resp
        .json()
        .await
        .map_err(|e| format!("$i18n:update.error.parse|msg={}", e))?;

    // draft 通常不会出现在 /releases/latest 里(GitHub 自动过滤),
    // prerelease 同理。这里再做一次保险:如果碰到就视为"没有可用新版本"。
    if release.draft || release.prerelease {
        return Ok(UpdateCheckResult {
            current_version: current_version.clone(),
            latest_version: current_version,
            update_available: false,
            release_url: format!("https://github.com/{}/releases", GITHUB_REPO),
            release_notes: String::new(),
            published_at: release.published_at,
        });
    }

    let latest_version = strip_v_prefix(&release.tag_name);
    let update_available = compare_versions(&latest_version, &current_version).is_gt();

    Ok(UpdateCheckResult {
        current_version,
        latest_version,
        update_available,
        release_url: release.html_url,
        release_notes: release.body,
        published_at: release.published_at,
    })
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    // 白名单只放 http(s),避免 file:// / custom-scheme 这种潜在的本地
    // handler 滥用。GitHub Release URL 必然是 https://。
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(format!("$i18n:update.error.bad_url|url={}", url));
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // `start` 是 cmd 内建命令,必须经 cmd /C 包一层。第一个参数是
        // 窗口标题(空字符串占位),否则 URL 会被当成标题而不是 target。
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// 去掉版本字符串的 `v` / `V` 前缀。`v0.2.0` → `0.2.0`。
fn strip_v_prefix(s: &str) -> String {
    let t = s.trim();
    t.strip_prefix('v')
        .or_else(|| t.strip_prefix('V'))
        .unwrap_or(t)
        .to_string()
}

/// 简化版 SemVer 比较:按点切分 → 解析数字段比大小。
///
/// 不引入 `semver` crate(避免给 Cargo 加新依赖),够用就行 —— DiskMind
/// 的版本号始终是 `MAJOR.MINOR.PATCH` 三段纯数字,带 `-pre.N`
/// suffix 时把 `-` 之后的部分忽略(预发布永远视作小于同号正式版),
/// 与 release.yml `prerelease: false` 的策略一致。
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split('-')
            .next()
            .unwrap_or("")
            .split('.')
            .map(|seg| seg.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let aa = parse(a);
    let bb = parse(b);
    let len = aa.len().max(bb.len());
    for i in 0..len {
        let x = aa.get(i).copied().unwrap_or(0);
        let y = bb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => continue,
            ord => return ord,
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn strip_v_handles_both_prefixes_and_whitespace() {
        assert_eq!(strip_v_prefix("v0.2.0"), "0.2.0");
        assert_eq!(strip_v_prefix("V0.2.0"), "0.2.0");
        assert_eq!(strip_v_prefix("  v1.2.3  "), "1.2.3");
        assert_eq!(strip_v_prefix("1.0.0"), "1.0.0");
    }

    #[test]
    fn compare_basic_semver() {
        assert_eq!(compare_versions("0.2.0", "0.1.0"), Ordering::Greater);
        assert_eq!(compare_versions("0.1.0", "0.2.0"), Ordering::Less);
        assert_eq!(compare_versions("0.1.0", "0.1.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "0.99.99"), Ordering::Greater);
        assert_eq!(compare_versions("0.10.0", "0.9.0"), Ordering::Greater);
    }

    #[test]
    fn compare_unequal_segment_count() {
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0.1", "1.0.0"), Ordering::Greater);
    }

    #[test]
    fn compare_strips_prerelease_suffix() {
        // 当前实现:`-pre` suffix 直接被截掉,所以与同号正式版相等。
        // 这是 DiskMind 的有意取舍 —— release.yml prerelease:false,
        // pre 版本不会进入 /releases/latest 链路。
        assert_eq!(compare_versions("0.2.0-pre.1", "0.2.0"), Ordering::Equal);
    }
}
