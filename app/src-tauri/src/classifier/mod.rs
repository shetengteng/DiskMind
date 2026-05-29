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

fn match_rule(entry: &FileEntry) -> Option<(&'static str, FileRisk, &'static str)> {
    let path_lower = entry.path.to_ascii_lowercase();
    let ext = entry.extension.to_ascii_lowercase();

    // ---- 浏览器缓存(Low Risk · 清理 100% 安全)----
    // 都走 bundle id / 路径段匹配,跨用户路径(Library/Caches 下不同应用)
    // 覆盖 macOS + 现代主流浏览器。`/library/caches/<id>` 是 macOS sandboxed
    // app 的标准缓存目录,清理只影响下次冷启动加载速度。
    if path_lower.contains("/library/caches/com.apple.safari") {
        return Some((
            "浏览器缓存",
            FileRisk::Low,
            "Safari 浏览器缓存,重启后自动重建,不影响书签和密码。",
        ));
    }
    if path_lower.contains("/library/caches/google/chrome")
        || path_lower.contains("/library/caches/com.google.chrome")
    {
        return Some(("浏览器缓存", FileRisk::Low, "Chrome 浏览器缓存,清理安全。"));
    }
    if path_lower.contains("/library/caches/firefox")
        || path_lower.contains("/library/caches/org.mozilla.firefox")
    {
        return Some(("浏览器缓存", FileRisk::Low, "Firefox 浏览器缓存,清理安全。"));
    }
    if path_lower.contains("/library/caches/com.microsoft.edgemac")
        || path_lower.contains("/library/caches/microsoft edge")
    {
        return Some(("浏览器缓存", FileRisk::Low, "Edge 浏览器缓存,清理安全。"));
    }
    if path_lower.contains("/library/caches/com.brave.browser")
        || path_lower.contains("/library/caches/com.operasoftware.opera")
        || path_lower.contains("/library/caches/company.thebrowser.browser")
        || path_lower.contains("/library/caches/com.vivaldi.vivaldi")
    {
        return Some(("浏览器缓存", FileRisk::Low, "Chromium 系浏览器缓存,清理安全。"));
    }

    // ---- 通讯应用缓存 / 数据(Low / Medium 视场景)----
    // bundle id 走 macOS Group Containers + Application Support 双路径,
    // 这些应用倾向把视频/媒体附件长期缓存,常见 GB 级别。
    if path_lower.contains("/library/caches/us.zoom.xos")
        || path_lower.contains("/library/application support/zoom.us/")
    {
        return Some((
            "通讯应用缓存",
            FileRisk::Low,
            "Zoom 客户端缓存与日志,清理后不影响登录态。",
        ));
    }
    if path_lower.contains("/library/application support/slack/")
        || path_lower.contains("/library/caches/com.tinyspeck.slackmacgap")
    {
        return Some((
            "通讯应用缓存",
            FileRisk::Low,
            "Slack 缓存与离线消息,清理后下次启动会重新拉取。",
        ));
    }
    if path_lower.contains("/library/application support/discord/")
        || path_lower.contains("/library/caches/com.hnc.discord")
    {
        return Some((
            "通讯应用缓存",
            FileRisk::Low,
            "Discord 缓存,清理后不影响账号与服务器列表。",
        ));
    }
    if path_lower.contains("/library/containers/com.tencent.xinwechat/")
        || path_lower.contains("/library/containers/com.tencent.weworkmac/")
    {
        return Some((
            "通讯应用缓存",
            FileRisk::Medium,
            "微信 / 企业微信本地数据,可能含聊天记录,确认有云备份再清理。",
        ));
    }
    if path_lower.contains("/library/application support/telegram/")
        || path_lower.contains("/library/group containers/group.org.telegram.")
    {
        return Some((
            "通讯应用缓存",
            FileRisk::Low,
            "Telegram 本地缓存,媒体已在云端,清理安全。",
        ));
    }

    // ---- 开发工具缓存(Low Risk · 全可重建)----
    if path_lower.contains("/node_modules/") {
        return Some((
            "开发产物",
            FileRisk::Low,
            "可通过 `pnpm install` / `npm install` 重新生成。",
        ));
    }
    if path_lower.contains("/library/developer/xcode/deriveddata") {
        return Some((
            "开发产物",
            FileRisk::Low,
            "Xcode 派生数据,删除后下次构建会自动重建。",
        ));
    }
    if path_lower.contains("/library/developer/coresimulator/") {
        return Some((
            "开发产物",
            FileRisk::Medium,
            "iOS 模拟器镜像,清理后需重新下载,建议先删除不用的 device。",
        ));
    }
    if path_lower.contains("/library/developer/xcode/ios devicesupport") {
        return Some((
            "开发产物",
            FileRisk::Low,
            "Xcode iOS DeviceSupport 符号文件,连接真机后会重新生成。",
        ));
    }
    if path_lower.contains("/.cargo/registry") || path_lower.contains("/.cargo/git") {
        return Some(("开发产物", FileRisk::Low, "Cargo 包缓存,可通过下载重建。"));
    }
    if path_lower.contains("/.gradle/caches") {
        return Some(("开发产物", FileRisk::Low, "Gradle 构建缓存,重新构建会自动重建。"));
    }
    if path_lower.contains("/.m2/repository") {
        return Some(("开发产物", FileRisk::Low, "Maven 本地仓库,重新构建会自动下载。"));
    }
    if path_lower.contains("/library/caches/jetbrains/")
        || path_lower.contains("/library/caches/com.jetbrains.")
        || path_lower.contains("/library/logs/jetbrains/")
    {
        return Some((
            "开发产物",
            FileRisk::Low,
            "JetBrains IDE 缓存与索引,清理后下次启动会重建索引。",
        ));
    }
    if path_lower.contains("/library/caches/com.microsoft.vscode")
        || path_lower.contains("/library/application support/code/cache")
        || path_lower.contains("/library/application support/code/cachedata")
    {
        return Some((
            "开发产物",
            FileRisk::Low,
            "VSCode 缓存,清理后下次启动重建,不影响配置与插件。",
        ));
    }
    if path_lower.contains("/library/containers/com.docker.docker/")
        || path_lower.contains("/library/group containers/group.com.docker/")
    {
        return Some((
            "开发产物",
            FileRisk::Medium,
            "Docker Desktop 数据卷,可能含正在使用的容器镜像,谨慎清理。",
        ));
    }
    if path_lower.contains("/.cache/pip/") || path_lower.contains("/.cache/yarn/") {
        return Some(("开发产物", FileRisk::Low, "包管理器缓存,可通过下载重建。"));
    }
    if path_lower.contains("/library/caches/go-build/") {
        return Some(("开发产物", FileRisk::Low, "Go 构建缓存,重新编译会自动重建。"));
    }

    // ---- 系统临时 / 元数据(Low Risk)----
    if path_lower.ends_with("/.ds_store") {
        return Some(("系统临时", FileRisk::Low, "macOS 目录元数据缓存,自动重建。"));
    }
    if path_lower.contains("/library/logs/") || (ext == "log" && entry.size > LOG_THRESHOLD) {
        return Some(("日志", FileRisk::Low, "应用日志文件,清理后会重新生成。"));
    }

    // ---- 用户数据 / 备份(Medium / High · 清理前需确认)----
    if path_lower.contains("/library/application support/mobilesync/backup/") {
        return Some((
            "iOS 备份",
            FileRisk::Medium,
            "iTunes / Finder 同步的 iOS 设备备份,清理前确认重要数据已迁移。",
        ));
    }
    if path_lower.contains("/library/caches/com.spotify.client") {
        return Some(("流媒体缓存", FileRisk::Low, "Spotify 本地缓存,清理后下次播放需重新下载。"));
    }

    // ---- 回收站残留 ----
    if path_lower.contains("/.trash/") {
        return Some((
            "回收站残留",
            FileRisk::Medium,
            "系统回收站残留,清空回收站后释放磁盘空间。",
        ));
    }

    // ---- 安装包 / 大型媒体 ----
    if matches!(ext.as_str(), "dmg" | "pkg" | "iso") && entry.size > 100 * 1024 * 1024 {
        return Some(("过期下载", FileRisk::Medium, "安装包,确认应用已安装后可清理。"));
    }
    if entry.size > LARGE_FILE_THRESHOLD
        && matches!(
            ext.as_str(),
            "mov" | "mp4" | "mkv" | "avi" | "wav" | "flac" | "psd" | "raw"
        )
    {
        return Some((
            "大型媒体",
            FileRisk::High,
            "大型媒体文件,建议保留或先备份到云端。",
        ));
    }

    if entry.size >= FALLBACK_LARGE_FILE {
        return Some((
            "待审查大文件",
            FileRisk::High,
            "未命中清理规则,但占用 >1GB,建议人工或 AI 复核。",
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
            ("/Users/x/Library/Caches/com.apple.Safari/data", "浏览器缓存"),
            ("/Users/x/Library/Caches/com.google.Chrome/Cache", "浏览器缓存"),
            ("/Users/x/Library/Caches/com.microsoft.edgemac/foo", "浏览器缓存"),
            ("/Users/x/Library/Caches/com.brave.Browser/foo", "浏览器缓存"),
            ("/Users/x/Library/Caches/company.thebrowser.Browser/foo", "浏览器缓存"),
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
            ("/Users/x/Library/Caches/JetBrains/IntelliJIdea2024.1/log", "开发产物"),
            ("/Users/x/Library/Application Support/Code/Cache/file", "开发产物"),
            ("/Users/x/Library/Developer/CoreSimulator/Devices/abc/data", "开发产物"),
            ("/Users/x/.gradle/caches/modules-2/files", "开发产物"),
            ("/Users/x/.m2/repository/com/example/foo.jar", "开发产物"),
            ("/Users/x/Library/Caches/go-build/00/file.o", "开发产物"),
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
            ("/Users/x/Library/Application Support/Slack/Cache/file", "通讯应用缓存"),
            ("/Users/x/Library/Application Support/discord/Cache", "通讯应用缓存"),
            ("/Users/x/Library/Containers/com.tencent.xinWeChat/Data", "通讯应用缓存"),
            ("/Users/x/Library/Application Support/zoom.us/data", "通讯应用缓存"),
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
        let (category, risk, _) = result.unwrap();
        assert_eq!(category, "iOS 备份");
        assert!(matches!(risk, FileRisk::Medium));
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
        assert_eq!(rows[0].category, "待审查大文件");
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
