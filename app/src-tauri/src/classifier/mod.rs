use crate::scanner::FileEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FileRisk {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResultRow {
    pub id: u64,
    pub path: String,
    pub category: String,
    pub size: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
    pub risk: FileRisk,
    #[serde(rename = "aiReason")]
    pub ai_reason: String,
    /// Round 20 · P0-1.2 增量扫描:文件 mtime(Unix epoch 秒)。`save_scan`
    /// 用 `(path, mtime, size_bytes)` 三元组与上次最新 run 命中复用,从而
    /// 跳过 ai_classify_batch 对未变化大文件的重复 LLM 调用。前端无消费方
    /// (用户不需要看到 mtime),所以 `skip_serializing` 不入 IPC payload。
    #[serde(default, skip_serializing)]
    pub mtime: u64,
    /// 该路径在加载快照时已不在原位 — 要么被 DiskMind 沙箱回收站
    /// 移走(`trash_item.status = 'in_trash'`),要么被用户在外部
    /// 直接删除/移动。前端据此过滤或灰显幽灵记录,避免出现 "扫描
    /// 结果里有,但 Finder 里找不到" 的诡异体验。
    ///
    /// 由 `Db::load_latest` 在返回前回填;实时 `scan:complete` 推送
    /// 时不会出现,因为那是刚刚扫出来的结果。新扫描入库时永远是
    /// `false`,所以 SQLite 无需新增列,仅在内存中标记。
    #[serde(default, skip_serializing_if = "is_false")]
    pub missing: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

const MIN_REPORT_SIZE: u64 = 1024 * 1024;
const LARGE_FILE_THRESHOLD: u64 = 500 * 1024 * 1024;
const LOG_THRESHOLD: u64 = 10 * 1024 * 1024;
const FALLBACK_LARGE_FILE: u64 = 1024 * 1024 * 1024;

/// 单次扫描保留的候选文件上限。按物理大小降序排序后,超出此数量的部分
/// 在写入 SQLite / 推到 UI 之前直接丢弃。设置上限是为了让结果表保持响
/// 应、并约束滚动历史窗口内的 DB 增长;调大会以渲染开销换取可见性。
const MAX_CANDIDATES_PER_RUN: usize = 2000;

pub fn classify(entries: Vec<FileEntry>) -> Vec<ScanResultRow> {
    let mut next_id: u64 = 1;
    let mut rows: Vec<ScanResultRow> = Vec::new();

    for entry in entries {
        if let Some((category, risk, reason)) = match_rule(&entry) {
            // 按逻辑大小(用户感知)过滤。
            if entry.size < MIN_REPORT_SIZE {
                continue;
            }

            // 上报字节数 = 物理(磁盘实际)占用。这是删除文件后可释放的
            // 数据量,也对应操作系统“已用空间”的口径。
            let report_bytes = if entry.phys_size > 0 {
                entry.phys_size
            } else {
                entry.size
            };

            rows.push(ScanResultRow {
                id: next_id,
                path: entry.path,
                category: category.to_string(),
                size: humanize_bytes(report_bytes),
                size_bytes: report_bytes,
                risk,
                ai_reason: reason.to_string(),
                mtime: entry.mtime,
                missing: false,
            });
            next_id += 1;
        }
    }

    rows.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    rows.truncate(MAX_CANDIDATES_PER_RUN);
    rows
}

// Round 26 · i18n 改造:
//
// 历史上 `match_rule` 返回的是 `(中文 category 名, FileRisk, 中文风险描
// 述)`,这两个 String 经 IPC 流到前端后无法跟随 vue-i18n locale 切换。
//
// 现在所有返回值都改成 **稳定标识符**:
// - category → English snake_case 短名(如 `browser_cache`),DB 直接存
//   这个值;前端通过 `category.<id>` 字典翻译显示
// - ai_reason → `$i18n:classifier.reason.<id>` marker,前端 `localize()`
//   识别前缀后调 `t()` 翻译
//
// 选择 stable ID 而非 marker 给 category 是因为它会作为 SQL JOIN /
// GROUP BY key 出现(分组聚合 / Reports 维度筛选),用 marker 字符串
// 含 `$i18n:` 前缀的 key 在 SQL 里不优雅;用扁平 ID 各业务可直接
// `.eq("browser_cache")` 比对。

fn match_rule(entry: &FileEntry) -> Option<(&'static str, FileRisk, &'static str)> {
    let path_lower = entry.path.to_ascii_lowercase();
    let ext = entry.extension.to_ascii_lowercase();

    // ---- 浏览器缓存(Low Risk · 清理 100% 安全)----
    if path_lower.contains("/library/caches/com.apple.safari") {
        return Some((
            "browser_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.browser_cache_safari",
        ));
    }
    if path_lower.contains("/library/caches/google/chrome")
        || path_lower.contains("/library/caches/com.google.chrome")
    {
        return Some((
            "browser_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.browser_cache_chrome",
        ));
    }
    if path_lower.contains("/library/caches/firefox")
        || path_lower.contains("/library/caches/org.mozilla.firefox")
    {
        return Some((
            "browser_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.browser_cache_firefox",
        ));
    }
    if path_lower.contains("/library/caches/com.microsoft.edgemac")
        || path_lower.contains("/library/caches/microsoft edge")
    {
        return Some((
            "browser_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.browser_cache_edge",
        ));
    }
    if path_lower.contains("/library/caches/com.brave.browser")
        || path_lower.contains("/library/caches/com.operasoftware.opera")
        || path_lower.contains("/library/caches/company.thebrowser.browser")
        || path_lower.contains("/library/caches/com.vivaldi.vivaldi")
    {
        return Some((
            "browser_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.browser_cache_chromium",
        ));
    }

    // ---- 通讯应用缓存 / 数据(Low / Medium 视场景)----
    if path_lower.contains("/library/caches/us.zoom.xos")
        || path_lower.contains("/library/application support/zoom.us/")
    {
        return Some((
            "messaging_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.messaging_cache_zoom",
        ));
    }
    if path_lower.contains("/library/application support/slack/")
        || path_lower.contains("/library/caches/com.tinyspeck.slackmacgap")
    {
        return Some((
            "messaging_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.messaging_cache_slack",
        ));
    }
    if path_lower.contains("/library/application support/discord/")
        || path_lower.contains("/library/caches/com.hnc.discord")
    {
        return Some((
            "messaging_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.messaging_cache_discord",
        ));
    }
    if path_lower.contains("/library/containers/com.tencent.xinwechat/")
        || path_lower.contains("/library/containers/com.tencent.weworkmac/")
    {
        return Some((
            "messaging_cache",
            FileRisk::Medium,
            "$i18n:classifier.reason.messaging_cache_wechat",
        ));
    }
    if path_lower.contains("/library/application support/telegram/")
        || path_lower.contains("/library/group containers/group.org.telegram.")
    {
        return Some((
            "messaging_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.messaging_cache_telegram",
        ));
    }

    // ---- 开发工具缓存(Low Risk · 全可重建)----
    if path_lower.contains("/node_modules/") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_node_modules",
        ));
    }
    if path_lower.contains("/library/developer/xcode/deriveddata") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_xcode_derived",
        ));
    }
    if path_lower.contains("/library/developer/coresimulator/") {
        return Some((
            "dev_artifacts",
            FileRisk::Medium,
            "$i18n:classifier.reason.dev_artifacts_ios_simulator",
        ));
    }
    if path_lower.contains("/library/developer/xcode/ios devicesupport") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_xcode_devicesupport",
        ));
    }
    if path_lower.contains("/.cargo/registry") || path_lower.contains("/.cargo/git") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_cargo",
        ));
    }
    if path_lower.contains("/.gradle/caches") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_gradle",
        ));
    }
    if path_lower.contains("/.m2/repository") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_maven",
        ));
    }
    if path_lower.contains("/library/caches/jetbrains/")
        || path_lower.contains("/library/caches/com.jetbrains.")
        || path_lower.contains("/library/logs/jetbrains/")
    {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_jetbrains",
        ));
    }
    if path_lower.contains("/library/caches/com.microsoft.vscode")
        || path_lower.contains("/library/application support/code/cache")
        || path_lower.contains("/library/application support/code/cachedata")
    {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_vscode",
        ));
    }
    if path_lower.contains("/library/containers/com.docker.docker/")
        || path_lower.contains("/library/group containers/group.com.docker/")
    {
        return Some((
            "dev_artifacts",
            FileRisk::Medium,
            "$i18n:classifier.reason.dev_artifacts_docker",
        ));
    }
    if path_lower.contains("/.cache/pip/") || path_lower.contains("/.cache/yarn/") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_pkg_cache",
        ));
    }
    if path_lower.contains("/library/caches/go-build/") {
        return Some((
            "dev_artifacts",
            FileRisk::Low,
            "$i18n:classifier.reason.dev_artifacts_go_build",
        ));
    }

    // ---- 系统临时 / 元数据(Low Risk)----
    if path_lower.ends_with("/.ds_store") {
        return Some((
            "system_temp",
            FileRisk::Low,
            "$i18n:classifier.reason.system_temp_ds_store",
        ));
    }
    if path_lower.contains("/library/logs/") || (ext == "log" && entry.size > LOG_THRESHOLD) {
        return Some((
            "logs",
            FileRisk::Low,
            "$i18n:classifier.reason.logs_app",
        ));
    }

    // ---- 用户数据 / 备份(Medium / High · 清理前需确认)----
    if path_lower.contains("/library/application support/mobilesync/backup/") {
        return Some((
            "ios_backup",
            FileRisk::Medium,
            "$i18n:classifier.reason.ios_backup",
        ));
    }
    if path_lower.contains("/library/caches/com.spotify.client") {
        return Some((
            "media_cache",
            FileRisk::Low,
            "$i18n:classifier.reason.media_cache_spotify",
        ));
    }

    // ---- 回收站残留 ----
    if path_lower.contains("/.trash/") {
        return Some((
            "trash_residue",
            FileRisk::Medium,
            "$i18n:classifier.reason.trash_residue",
        ));
    }

    // ---- 安装包 / 大型媒体 ----
    if matches!(ext.as_str(), "dmg" | "pkg" | "iso") && entry.size > 100 * 1024 * 1024 {
        return Some((
            "expired_download",
            FileRisk::Medium,
            "$i18n:classifier.reason.expired_download_installer",
        ));
    }
    if entry.size > LARGE_FILE_THRESHOLD
        && matches!(
            ext.as_str(),
            "mov" | "mp4" | "mkv" | "avi" | "wav" | "flac" | "psd" | "raw"
        )
    {
        return Some((
            "large_media",
            FileRisk::High,
            "$i18n:classifier.reason.large_media",
        ));
    }

    if entry.size >= FALLBACK_LARGE_FILE {
        return Some((
            "review_large",
            FileRisk::High,
            "$i18n:classifier.reason.review_large",
        ));
    }

    None
}

fn humanize_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_basic() {
        assert_eq!(humanize_bytes(0), "0 B");
        assert_eq!(humanize_bytes(2048), "2 KB");
        assert!(humanize_bytes(1024 * 1024 * 5).starts_with("5"));
    }

    /// 构造一个 FileEntry stub,用于 match_rule 的输入。
    fn stub_entry(path: &str, size: u64, ext: &str) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            size,
            phys_size: size,
            extension: ext.to_string(),
            mtime: 0,
            is_symlink: false,
            dev: 0,
            inode: 0,
        }
    }

    /// 新增 bundle id 映射的回归测试 — 校验 Round 21 扩充的 19 条规则
    /// 都能正确命中,避免后续重排顺序时被某条更宽松的规则误吞。
    #[test]
    fn bundle_id_browser_cache_rules() {
        let cases = [
            ("/Users/x/Library/Caches/com.apple.Safari/data", "browser_cache"),
            ("/Users/x/Library/Caches/com.google.Chrome/Cache", "browser_cache"),
            ("/Users/x/Library/Caches/com.microsoft.edgemac/foo", "browser_cache"),
            ("/Users/x/Library/Caches/com.brave.Browser/foo", "browser_cache"),
            ("/Users/x/Library/Caches/company.thebrowser.Browser/foo", "browser_cache"),
        ];
        for (path, expected) in cases {
            let entry = stub_entry(path, MIN_REPORT_SIZE + 1, "");
            let result = match_rule(&entry);
            assert!(
                result.is_some(),
                "expected match for {path}, got None"
            );
            assert_eq!(result.unwrap().0, expected, "wrong category for {path}");
        }
    }

    #[test]
    fn bundle_id_dev_tools_rules() {
        let cases = [
            ("/Users/x/Library/Caches/JetBrains/IntelliJIdea2024.1/log", "dev_artifacts"),
            ("/Users/x/Library/Application Support/Code/Cache/file", "dev_artifacts"),
            ("/Users/x/Library/Developer/CoreSimulator/Devices/abc/data", "dev_artifacts"),
            ("/Users/x/.gradle/caches/modules-2/files", "dev_artifacts"),
            ("/Users/x/.m2/repository/com/example/foo.jar", "dev_artifacts"),
            ("/Users/x/Library/Caches/go-build/00/file.o", "dev_artifacts"),
        ];
        for (path, expected) in cases {
            let entry = stub_entry(path, MIN_REPORT_SIZE + 1, "");
            let result = match_rule(&entry);
            assert!(result.is_some(), "expected match for {path}, got None");
            assert_eq!(result.unwrap().0, expected, "wrong category for {path}");
        }
    }

    #[test]
    fn bundle_id_communication_apps() {
        let cases = [
            ("/Users/x/Library/Application Support/Slack/Cache/file", "messaging_cache"),
            ("/Users/x/Library/Application Support/discord/Cache", "messaging_cache"),
            ("/Users/x/Library/Containers/com.tencent.xinWeChat/Data", "messaging_cache"),
            ("/Users/x/Library/Application Support/zoom.us/data", "messaging_cache"),
        ];
        for (path, expected) in cases {
            let entry = stub_entry(path, MIN_REPORT_SIZE + 1, "");
            let result = match_rule(&entry);
            assert!(result.is_some(), "expected match for {path}, got None");
            assert_eq!(result.unwrap().0, expected, "wrong category for {path}");
        }
    }

    #[test]
    fn ios_backup_recognized() {
        let entry = stub_entry(
            "/Users/x/Library/Application Support/MobileSync/Backup/abc123/Manifest.db",
            MIN_REPORT_SIZE + 1,
            "db",
        );
        let result = match_rule(&entry);
        assert!(result.is_some());
        let (category, risk, reason) = result.unwrap();
        assert_eq!(category, "ios_backup");
        assert!(matches!(risk, FileRisk::Medium));
        // ai_reason 必须是 i18n marker 而非裸中文 — Round 26 i18n 契约。
        assert!(reason.starts_with("$i18n:classifier.reason."));
    }

    // ---- classify 主入口端到端测试 -------------------------------------
    // 关注三件事:
    // 1. 排序契约:输出按 size_bytes DESC,这是前端 Top N 卡片的隐含依赖
    // 2. 过滤契约:size < MIN_REPORT_SIZE 的不入选,phys_size 优先于 size
    // 3. 截断契约:超过 MAX_CANDIDATES_PER_RUN 的丢弃,保护 DB 与 UI 响应
    // 4. fallback 兜底:>1GB 即使未命中规则也入选,防漏

    #[test]
    fn classify_sorts_by_size_desc() {
        let entries = vec![
            stub_entry("/a/node_modules/foo", 5 * 1024 * 1024, ""),
            stub_entry("/b/node_modules/bar", 50 * 1024 * 1024, ""),
            stub_entry("/c/node_modules/baz", 1 * 1024 * 1024, ""),
        ];
        let rows = classify(entries);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].size_bytes, 50 * 1024 * 1024);
        assert_eq!(rows[1].size_bytes, 5 * 1024 * 1024);
        assert_eq!(rows[2].size_bytes, 1 * 1024 * 1024);
    }

    #[test]
    fn classify_filters_under_min_size() {
        // < 1MB 命中规则也不入选 — 节约用户视线
        let entry = stub_entry("/library/caches/com.apple.safari/x", 100 * 1024, "");
        let rows = classify(vec![entry]);
        assert!(rows.is_empty(), "files under MIN_REPORT_SIZE must be filtered");
    }

    #[test]
    fn classify_prefers_phys_size_when_present() {
        // 稀疏文件:size=10MB 但 phys_size=2MB,上报应该用 2MB
        let mut entry = stub_entry("/x/node_modules/foo", 10 * 1024 * 1024, "");
        entry.phys_size = 2 * 1024 * 1024;
        let rows = classify(vec![entry]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].size_bytes, 2 * 1024 * 1024);
    }

    #[test]
    fn classify_falls_back_to_size_when_phys_is_zero() {
        let mut entry = stub_entry("/x/node_modules/foo", 5 * 1024 * 1024, "");
        entry.phys_size = 0;
        let rows = classify(vec![entry]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].size_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn classify_keeps_large_file_without_matching_rule() {
        // 路径未命中任一规则,但 size > 1GB → fallback 大文件审查
        let entry = stub_entry("/users/x/unknown.bin", FALLBACK_LARGE_FILE + 1, "bin");
        let rows = classify(vec![entry]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category, "review_large");
        assert!(matches!(rows[0].risk, FileRisk::High));
    }

    #[test]
    fn classify_truncates_to_max_candidates() {
        // 注意:MAX_CANDIDATES_PER_RUN 是 2000,这里直接造 2050 个超过上限
        let mut entries = Vec::with_capacity(2050);
        for i in 0..2050 {
            entries.push(stub_entry(
                &format!("/x/node_modules/lib-{i}"),
                MIN_REPORT_SIZE * (i as u64 + 1),
                "",
            ));
        }
        let rows = classify(entries);
        assert_eq!(
            rows.len(),
            MAX_CANDIDATES_PER_RUN,
            "must truncate to MAX_CANDIDATES_PER_RUN"
        );
        // 截断后保留 top N(最大的),并按 desc 排序
        assert!(
            rows[0].size_bytes > rows[rows.len() - 1].size_bytes,
            "truncation should keep largest entries (sorted desc)"
        );
    }

    #[test]
    fn classify_carries_mtime_through() {
        // 增量扫描复用依赖 mtime 字段透传,不能漏
        let mut entry = stub_entry("/x/node_modules/foo", MIN_REPORT_SIZE + 1, "");
        entry.mtime = 1_700_000_000;
        let rows = classify(vec![entry]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mtime, 1_700_000_000);
    }
}
