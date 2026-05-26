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
}

const MIN_REPORT_SIZE: u64 = 1024 * 1024;
const LARGE_FILE_THRESHOLD: u64 = 500 * 1024 * 1024;
const LOG_THRESHOLD: u64 = 10 * 1024 * 1024;
const FALLBACK_LARGE_FILE: u64 = 1024 * 1024 * 1024;

pub fn classify(entries: Vec<FileEntry>) -> Vec<ScanResultRow> {
    let mut next_id: u64 = 1;
    let mut rows: Vec<ScanResultRow> = Vec::new();

    for entry in entries {
        if let Some((category, risk, reason)) = match_rule(&entry) {
            // Filter by logical size (what the user thinks the file is).
            if entry.size < MIN_REPORT_SIZE {
                continue;
            }

            // Reportable bytes = physical (on-disk) usage. This is what gets
            // freed if the file is removed and matches "Used" reported by the OS.
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
            });
            next_id += 1;
        }
    }

    rows.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    rows.truncate(500);
    rows
}

fn match_rule(entry: &FileEntry) -> Option<(&'static str, FileRisk, &'static str)> {
    let path_lower = entry.path.to_ascii_lowercase();
    let ext = entry.extension.to_ascii_lowercase();

    if path_lower.contains("/library/caches/com.apple.safari") {
        return Some((
            "浏览器缓存",
            FileRisk::Low,
            "Safari 浏览器缓存,重启后自动重建,不影响书签和密码。",
        ));
    }
    if path_lower.contains("/library/caches/google/chrome") {
        return Some(("浏览器缓存", FileRisk::Low, "Chrome 浏览器缓存,清理安全。"));
    }
    if path_lower.contains("/library/caches/firefox") {
        return Some(("浏览器缓存", FileRisk::Low, "Firefox 浏览器缓存,清理安全。"));
    }
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
    if path_lower.contains("/.cargo/registry") || path_lower.contains("/.cargo/git") {
        return Some(("开发产物", FileRisk::Low, "Cargo 包缓存,可通过下载重建。"));
    }
    if path_lower.ends_with("/.ds_store") {
        return Some(("系统临时", FileRisk::Low, "macOS 目录元数据缓存,自动重建。"));
    }
    if path_lower.contains("/library/logs/") || (ext == "log" && entry.size > LOG_THRESHOLD) {
        return Some(("日志", FileRisk::Low, "应用日志文件,清理后会重新生成。"));
    }
    if path_lower.contains("/.trash/") {
        return Some((
            "回收站残留",
            FileRisk::Medium,
            "系统回收站残留,清空回收站后释放磁盘空间。",
        ));
    }
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
}
