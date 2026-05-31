//! AI Tagging Engine — 文件标签分级引擎。
//!
//! 三级 fallback:
//!   L1 · 规则引擎:扩展名 + 路径模式,~80% 覆盖率,0 成本
//!   L2 · 本地 embedding(暂不实现,预留)
//!   L3 · LLM fallback:batch 请求,5% 兜底
//!
//! 缓存:标签结果写入 `ai_tag_cache` 表(path_hash 做 key),7 天过期。

use std::path::Path;

/// L1 规则引擎的标签输出。
pub struct RuleTagResult {
    pub label: String,
    pub category: TagCategory,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagCategory {
    Work,
    Media,
    Archive,
    Temp,
    Code,
    System,
    Other,
}

impl TagCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Media => "media",
            Self::Archive => "archive",
            Self::Temp => "temp",
            Self::Code => "code",
            Self::System => "system",
            Self::Other => "other",
        }
    }
}

/// L1 规则引擎:根据文件扩展名和路径模式判定标签。
/// 返回 `None` 表示规则未命中,应走 L3 LLM fallback。
pub fn tag_by_rule(path: &str, extension: &str) -> Option<RuleTagResult> {
    let ext = extension.to_lowercase();
    let path_lower = path.to_lowercase();

    // ── Temp / 可清理 ──
    if matches!(
        ext.as_str(),
        "tmp" | "bak" | "swp" | "swo" | "log" | "cache" | "crdownload" | "part"
    ) {
        return Some(RuleTagResult {
            label: "临时文件".into(),
            category: TagCategory::Temp,
            confidence: 0.95,
        });
    }

    if is_known_temp_name(&path_lower) {
        return Some(RuleTagResult {
            label: "临时文件".into(),
            category: TagCategory::Temp,
            confidence: 0.9,
        });
    }

    if is_cache_dir_child(&path_lower) {
        return Some(RuleTagResult {
            label: "缓存".into(),
            category: TagCategory::Temp,
            confidence: 0.85,
        });
    }

    // ── Archive / 安装包 ──
    if matches!(
        ext.as_str(),
        "dmg" | "pkg" | "exe" | "msi" | "deb" | "rpm" | "appimage" | "snap" | "apk" | "ipa"
    ) {
        return Some(RuleTagResult {
            label: "安装包".into(),
            category: TagCategory::Archive,
            confidence: 0.95,
        });
    }

    if matches!(
        ext.as_str(),
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" | "zst"
    ) {
        return Some(RuleTagResult {
            label: "压缩文件".into(),
            category: TagCategory::Archive,
            confidence: 0.9,
        });
    }

    // ── Media ──
    if matches!(
        ext.as_str(),
        "mp4" | "mov" | "mkv" | "avi" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
    ) {
        return Some(RuleTagResult {
            label: "视频".into(),
            category: TagCategory::Media,
            confidence: 0.95,
        });
    }

    if matches!(
        ext.as_str(),
        "mp3" | "wav" | "aac" | "flac" | "ogg" | "wma" | "m4a" | "opus"
    ) {
        return Some(RuleTagResult {
            label: "音频".into(),
            category: TagCategory::Media,
            confidence: 0.95,
        });
    }

    if matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "heic"
            | "heif" | "raw" | "cr2" | "nef" | "psd" | "ai"
    ) {
        return Some(RuleTagResult {
            label: "图片".into(),
            category: TagCategory::Media,
            confidence: 0.95,
        });
    }

    // ── Work / 文档 ──
    if matches!(
        ext.as_str(),
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
            | "rtf" | "pages" | "numbers" | "key" | "epub"
    ) {
        return Some(RuleTagResult {
            label: "文档".into(),
            category: TagCategory::Work,
            confidence: 0.9,
        });
    }

    if matches!(ext.as_str(), "txt" | "md" | "csv") {
        return Some(RuleTagResult {
            label: "文本".into(),
            category: TagCategory::Work,
            confidence: 0.8,
        });
    }

    // ── 开发产物(路径模式匹配,优先于扩展名匹配）──
    if is_dev_artifact_path(&path_lower) {
        return Some(RuleTagResult {
            label: "开发产物".into(),
            category: TagCategory::Temp,
            confidence: 0.9,
        });
    }

    // ── Code ──
    if matches!(
        ext.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "java"
            | "go"
            | "c"
            | "cpp"
            | "h"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "kt"
            | "scala"
            | "vue"
            | "svelte"
            | "html"
            | "css"
            | "scss"
            | "less"
            | "sql"
    ) {
        return Some(RuleTagResult {
            label: "代码".into(),
            category: TagCategory::Code,
            confidence: 0.9,
        });
    }

    if matches!(
        ext.as_str(),
        "json" | "yaml" | "yml" | "toml" | "xml" | "sh" | "bash" | "ini" | "cfg" | "conf"
            | "env" | "lock"
    ) {
        return Some(RuleTagResult {
            label: "配置".into(),
            category: TagCategory::Code,
            confidence: 0.8,
        });
    }

    // ── System ──
    if is_system_file(&path_lower, &ext) {
        return Some(RuleTagResult {
            label: "系统文件".into(),
            category: TagCategory::System,
            confidence: 0.85,
        });
    }

    None
}

fn is_known_temp_name(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    matches!(
        name,
        ".ds_store"
            | "thumbs.db"
            | "desktop.ini"
            | ".localized"
            | ".spotlight-v100"
            | ".trashes"
            | ".fseventsd"
            | ".temporaryitems"
    )
}

fn is_cache_dir_child(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    segments.iter().any(|s| {
        matches!(
            *s,
            "cache" | "caches" | "__pycache__" | ".cache" | "cacheddata"
        )
    })
}

fn is_dev_artifact_path(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    segments.iter().any(|s| {
        matches!(
            *s,
            "node_modules"
                | "target"
                | ".gradle"
                | "build"
                | "dist"
                | ".next"
                | ".nuxt"
                | "deriveddata"
                | ".tox"
                | ".mypy_cache"
                | ".pytest_cache"
                | "__pycache__"
                | ".venv"
                | "venv"
        )
    })
}

fn is_system_file(path: &str, ext: &str) -> bool {
    if matches!(ext, "sys" | "dll" | "dylib" | "so" | "framework") {
        return true;
    }
    path.contains("/library/") && path.contains("/application support/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l1_tags_video() {
        let r = tag_by_rule("/Users/test/Downloads/movie.mp4", "mp4").unwrap();
        assert_eq!(r.category, TagCategory::Media);
        assert_eq!(r.label, "视频");
    }

    #[test]
    fn l1_tags_installer() {
        let r = tag_by_rule("/Users/test/Downloads/app.dmg", "dmg").unwrap();
        assert_eq!(r.category, TagCategory::Archive);
        assert_eq!(r.label, "安装包");
    }

    #[test]
    fn l1_tags_temp() {
        let r = tag_by_rule("/Users/test/.DS_Store", "").unwrap();
        assert_eq!(r.category, TagCategory::Temp);
    }

    #[test]
    fn l1_tags_cache_child() {
        let r = tag_by_rule("/Users/test/Library/Caches/com.apple.Safari/data.bin", "bin").unwrap();
        assert_eq!(r.category, TagCategory::Temp);
        assert_eq!(r.label, "缓存");
    }

    #[test]
    fn l1_tags_code() {
        let r = tag_by_rule("/Users/test/project/main.rs", "rs").unwrap();
        assert_eq!(r.category, TagCategory::Code);
    }

    #[test]
    fn l1_returns_none_for_unknown() {
        let r = tag_by_rule("/Users/test/random_file.xyz", "xyz");
        assert!(r.is_none());
    }

    #[test]
    fn l1_tags_dev_artifacts() {
        let r = tag_by_rule("/Users/test/project/node_modules/pkg/index.js", "js").unwrap();
        assert_eq!(r.category, TagCategory::Temp);
        assert_eq!(r.label, "开发产物");
    }
}
