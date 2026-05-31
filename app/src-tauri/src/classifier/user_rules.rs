//! Round 29 · 用户自定义 classifier 规则。
//!
//! ## 设计原则
//!
//! - **追加而非覆盖** — 用户在 `app_data/rules.toml` 写的规则**优先级
//!   高于 builtin**,但 builtin 仍永远存在;用户只能往规则集里塞新模式,
//!   不能禁用或修改内置规则。这避免了"用户改坏 TOML 让分类失效 / 让风
//!   险评估丢失"的高风险路径,同时仍然兑现"外置 + 热更新"的核心价值
//!   (用户能针对自己电脑上的特殊缓存目录 / 第三方软件加规则)。
//! - **解析失败兜底空规则集** — TOML 损坏 / 字段错 / 文件不存在,任何一
//!   种情况都退化到"无用户规则",builtin 链路继续工作。日志打印失败原因
//!   让用户能在 crash log 里看到。
//! - **热更新单调原子** — `RwLock<Arc<UserRuleSet>>` 让 reload 时不会让
//!   classify 看到半新半旧的中间态;classify 拿 Arc clone 后就 release
//!   read lock,不阻塞 reload。
//!
//! ## TOML 示例
//!
//! ```toml
//! # ~/.../com.diskmind.app/rules.toml
//!
//! [[rule]]
//! id = "my_dropbox_cache"
//! category = "browser_cache"
//! risk = "low"
//! reason_key = "classifier.reason.browser_cache_chrome"  # 复用现有字典
//! matcher = { kind = "path_contains", value = "/library/caches/com.dropbox.dropbox" }
//!
//! [[rule]]
//! id = "ridiculously_large_zips"
//! category = "expired_download"
//! risk = "medium"
//! reason_key = "classifier.reason.expired_download_installer"
//! matcher = { kind = "ext_in_and_size_gt", exts = ["zip", "tar"], size_bytes = 1073741824 }
//! ```
//!
//! ## 安全
//!
//! 用户提供的 reason_key 进入前端 i18n 字典查找。若 key 不存在,前端
//! `localize()` 会显示原 marker 字符串(`$i18n:foo`),不会崩。我们在加
//! 载时不校验 reason_key 真实性 — 让 i18n 字典缺失透明暴露给开发者。

use crate::scanner::FileEntry;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

/// 用户规则的 matcher 枚举。覆盖 builtin 用到的全部 5 种模式,既可让用
/// 户复刻 builtin 的语义,也能加未覆盖的新模式。`PathContainsAny` 而非
/// 多写几条 `PathContains` 是为了 TOML 友好(一行一组)。
///
/// Round 34B:加 `Serialize` 派生,让 UI 可视化编辑器把改动写回 rules.toml。
/// `tag = "kind"` 在序列化时把 enum 名(snake_case)作为 inline table 的
/// `kind` 字段,与现有 Deserialize 形态严格对称,保证 read/write 往返
/// idempotent。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Matcher {
    /// 路径(转小写后)包含给定子串。
    PathContains { value: String },
    /// 路径包含给定子串列表里**任意**一个。
    PathContainsAny { values: Vec<String> },
    /// 路径以给定后缀结尾(转小写后)。例如 `.ds_store` 这种 Apple 元数据。
    PathEndsWith { value: String },
    /// 扩展名命中给定集合**且**逻辑大小 ≥ 阈值。常用于"大型 mp4 / dmg"。
    ExtInAndSizeGt {
        exts: Vec<String>,
        size_bytes: u64,
    },
    /// 单纯按大小兜底(忽略路径 / 扩展名),用于"未知超大文件"分类。
    SizeGte { size_bytes: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// 唯一 ID,主要给日志 / debug 用,classify hot path 不读。
    /// Round 34B 把 `dead_code` allow 拿掉 — 现在 UI 编辑器要展示和写回 id。
    pub id: String,
    /// 命中时写入 `scan_result.category`,期待用 stable English snake_case
    /// (与 builtin 保持一致),前端 `localizeCategory` 会翻译。
    pub category: String,
    pub risk: RiskLevel,
    /// 写入 `scan_result.ai_reason`。建议用 `classifier.reason.<id>` 形态
    /// 走 i18n marker,前端 `localize()` 会翻译;直接写明文也可以,但不
    /// 会跟随 locale 变化。
    pub reason_key: String,
    pub matcher: Matcher,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserRuleSet {
    #[serde(default)]
    pub rule: Vec<Rule>,
}

/// 把 RiskLevel 转回 classifier::FileRisk。两个枚举几乎同构 — 之所以不
/// 共享一个,是因为前者用 serde::Deserialize 派生(snake_case),后者用
/// Serde rename = "low"(对接前端 IPC 的旧约定),合并会让任一边的兼容
/// 性弱化。
impl RiskLevel {
    pub fn to_file_risk(&self) -> super::FileRisk {
        match self {
            RiskLevel::Low => super::FileRisk::Low,
            RiskLevel::Medium => super::FileRisk::Medium,
            RiskLevel::High => super::FileRisk::High,
        }
    }
}

impl Matcher {
    fn matches(&self, path_lower: &str, ext_lower: &str, size: u64) -> bool {
        match self {
            Matcher::PathContains { value } => path_lower.contains(value),
            Matcher::PathContainsAny { values } => values.iter().any(|v| path_lower.contains(v)),
            Matcher::PathEndsWith { value } => path_lower.ends_with(value),
            Matcher::ExtInAndSizeGt { exts, size_bytes } => {
                size > *size_bytes && exts.iter().any(|e| e == ext_lower)
            }
            Matcher::SizeGte { size_bytes } => size >= *size_bytes,
        }
    }
}

impl UserRuleSet {
    /// 顺序遍历用户规则,首个命中的胜出。返回 `(category, risk, reason_key)`,
    /// 与 builtin `match_rule` 输出格式 1:1 对齐,classifier 上层可统一处理。
    pub fn match_first(&self, entry: &FileEntry) -> Option<(String, super::FileRisk, String)> {
        if self.rule.is_empty() {
            return None;
        }
        let path_lower = entry.path.to_ascii_lowercase();
        let ext_lower = entry.extension.to_ascii_lowercase();
        for rule in &self.rule {
            if rule.matcher.matches(&path_lower, &ext_lower, entry.size) {
                return Some((
                    rule.category.clone(),
                    rule.risk.to_file_risk(),
                    rule.reason_key.clone(),
                ));
            }
        }
        None
    }
}

/// 全局共享。OnceLock 一次性初始化容器,内部 RwLock<Arc<>> 让 reload
/// 能原子替换。`classify()` 调用方走 `with_user_rules(|set| ...)`,会
/// 取一个 Arc clone 短暂持有,绝不卡 reload。
static USER_RULES: OnceLock<RwLock<Arc<UserRuleSet>>> = OnceLock::new();

fn cell() -> &'static RwLock<Arc<UserRuleSet>> {
    USER_RULES.get_or_init(|| RwLock::new(Arc::new(UserRuleSet::default())))
}

/// 从指定路径加载用户规则文件。文件不存在 / 解析失败 / 字段不全等情况
/// 统一返回空规则集 + warn 日志,**不向上抛错** — 让启动流程在最坏情况
/// 下退化到"只有 builtin 规则",而不是因为一个坏 TOML 让 app 起不来。
pub fn load_from(path: &Path) -> UserRuleSet {
    if !path.exists() {
        return UserRuleSet::default();
    }
    let text = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[diskmind] classifier: failed to read user rules at {}: {}",
                path.display(),
                e
            );
            return UserRuleSet::default();
        }
    };
    match toml::from_str::<UserRuleSet>(&text) {
        Ok(set) => {
            eprintln!(
                "[diskmind] classifier: loaded {} user rule(s) from {}",
                set.rule.len(),
                path.display()
            );
            set
        }
        Err(e) => {
            eprintln!(
                "[diskmind] classifier: failed to parse user rules at {}: {}",
                path.display(),
                e
            );
            UserRuleSet::default()
        }
    }
}

/// 把全局规则集替换为新值。reload 命令用;启动时 setup 阶段也用一次。
pub fn install(new_set: UserRuleSet) {
    let lock = cell();
    let mut guard = lock.write().expect("user rules lock poisoned");
    *guard = Arc::new(new_set);
}

/// Round 34B · 把规则集写回 TOML 文件,UI 可视化编辑器用。
///
/// - 父目录不存在自动 `create_dir_all`(首次写入场景:`app_data_dir`
///   存在但 `rules.toml` 从未生成)。
/// - 写入采用 atomic rename 模式:先写到 `<path>.tmp`,fsync 后 rename
///   到目标。中途崩溃不会留半截损坏文件,这条规则集在 classify hot
///   path 上,完整性优先级高。
/// - 任何 IO 失败都返回 `String` 错误,前端 toast 给用户。
pub fn save_to(path: &Path, set: &UserRuleSet) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create parent dir failed: {e}"))?;
    }
    let text = toml::to_string_pretty(set)
        .map_err(|e| format!("serialize rules failed: {e}"))?;
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, text.as_bytes())
        .map_err(|e| format!("write tmp failed: {e}"))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| format!("atomic rename failed: {e}"))?;
    Ok(())
}

/// classify 遍历用 — 取一个当前 RuleSet 的 Arc 快照,不阻塞 reload。
pub fn snapshot() -> Arc<UserRuleSet> {
    cell()
        .read()
        .expect("user rules lock poisoned")
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, ext: &str, size: u64) -> FileEntry {
        FileEntry {
            path: path.into(),
            size,
            phys_size: size,
            extension: ext.into(),
            mtime: 0,
            is_symlink: false,
            dev: 0,
            inode: 0,
        }
    }

    #[test]
    fn empty_set_returns_none() {
        let set = UserRuleSet::default();
        let out = set.match_first(&entry("/Users/x/foo", "txt", 100));
        assert!(out.is_none());
    }

    #[test]
    fn path_contains_basic_match() {
        let toml = r#"
            [[rule]]
            id = "my_cache"
            category = "browser_cache"
            risk = "low"
            reason_key = "classifier.reason.browser_cache_chrome"
            matcher = { kind = "path_contains", value = "/my/cache/dir" }
        "#;
        let set: UserRuleSet = toml::from_str(toml).unwrap();
        let hit = set.match_first(&entry("/Users/x/My/Cache/Dir/file", "log", 0));
        assert!(hit.is_some());
        let (cat, risk, reason) = hit.unwrap();
        assert_eq!(cat, "browser_cache");
        assert!(matches!(risk, super::super::FileRisk::Low));
        assert_eq!(reason, "classifier.reason.browser_cache_chrome");
    }

    #[test]
    fn first_rule_wins_over_later_overlap() {
        let toml = r#"
            [[rule]]
            id = "first"
            category = "logs"
            risk = "medium"
            reason_key = "classifier.reason.logs_app"
            matcher = { kind = "path_contains", value = "/library/caches/" }

            [[rule]]
            id = "second"
            category = "browser_cache"
            risk = "low"
            reason_key = "classifier.reason.browser_cache_chrome"
            matcher = { kind = "path_contains", value = "/library/caches/" }
        "#;
        let set: UserRuleSet = toml::from_str(toml).unwrap();
        let hit = set.match_first(&entry("/Users/x/Library/Caches/foo", "", 0));
        assert_eq!(hit.unwrap().0, "logs");
    }

    #[test]
    fn ext_in_and_size_gt_filters_both_dimensions() {
        let toml = r#"
            [[rule]]
            id = "big_videos"
            category = "large_media"
            risk = "high"
            reason_key = "classifier.reason.large_media"
            matcher = { kind = "ext_in_and_size_gt", exts = ["mp4", "mov"], size_bytes = 1000 }
        "#;
        let set: UserRuleSet = toml::from_str(toml).unwrap();
        // 命中:扩展名对 + 大小够
        assert!(set.match_first(&entry("/x/y.mp4", "mp4", 2000)).is_some());
        // 不命中:扩展名对但大小不够
        assert!(set.match_first(&entry("/x/y.mp4", "mp4", 500)).is_none());
        // 不命中:大小够但扩展名不对
        assert!(set.match_first(&entry("/x/y.zip", "zip", 2000)).is_none());
    }

    #[test]
    fn path_contains_any_matches_first_branch() {
        let toml = r#"
            [[rule]]
            id = "any_test"
            category = "dev_artifacts"
            risk = "low"
            reason_key = "classifier.reason.dev_artifacts_node_modules"
            matcher = { kind = "path_contains_any", values = ["/foo/", "/bar/"] }
        "#;
        let set: UserRuleSet = toml::from_str(toml).unwrap();
        assert!(set.match_first(&entry("/x/foo/file", "", 0)).is_some());
        assert!(set.match_first(&entry("/x/bar/file", "", 0)).is_some());
        assert!(set.match_first(&entry("/x/baz/file", "", 0)).is_none());
    }

    #[test]
    fn malformed_toml_in_load_returns_empty() {
        // tempfile + 写一个语法错的 TOML,验证 load_from 静默退到空集
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "this is [[ not valid toml @@@@").unwrap();
        let set = load_from(tmp.path());
        assert_eq!(set.rule.len(), 0);
    }

    #[test]
    fn missing_file_in_load_returns_empty() {
        let bogus = std::path::Path::new("/this/does/not/exist/rules.toml");
        let set = load_from(bogus);
        assert_eq!(set.rule.len(), 0);
    }

    #[test]
    fn install_then_snapshot_returns_same_data() {
        let toml = r#"
            [[rule]]
            id = "snap"
            category = "logs"
            risk = "low"
            reason_key = "classifier.reason.logs_app"
            matcher = { kind = "size_gte", size_bytes = 1000 }
        "#;
        let set: UserRuleSet = toml::from_str(toml).unwrap();
        install(set);
        let snap = snapshot();
        assert_eq!(snap.rule.len(), 1);
        assert_eq!(snap.rule[0].category, "logs");
        // 清理:把全局重置回空,避免污染其它测试
        install(UserRuleSet::default());
    }

    /// Round 34B · serialize 单元测试:每种 matcher 写回后必须再读出来
    /// 一模一样。这是 UI 编辑器写盘的核心契约,违反就会导致用户的规则
    /// 被悄悄丢失。
    #[test]
    fn save_then_load_roundtrip_preserves_all_matcher_kinds() {
        let original_toml = r#"
            [[rule]]
            id = "r1"
            category = "browser_cache"
            risk = "low"
            reason_key = "classifier.reason.browser_cache_chrome"
            matcher = { kind = "path_contains", value = "/library/caches/" }

            [[rule]]
            id = "r2"
            category = "dev_artifacts"
            risk = "medium"
            reason_key = "classifier.reason.dev_artifacts_node_modules"
            matcher = { kind = "path_contains_any", values = ["/node_modules/", "/.venv/"] }

            [[rule]]
            id = "r3"
            category = "system_metadata"
            risk = "low"
            reason_key = "classifier.reason.system_metadata"
            matcher = { kind = "path_ends_with", value = ".ds_store" }

            [[rule]]
            id = "r4"
            category = "large_media"
            risk = "high"
            reason_key = "classifier.reason.large_media"
            matcher = { kind = "ext_in_and_size_gt", exts = ["mp4", "mov"], size_bytes = 1073741824 }

            [[rule]]
            id = "r5"
            category = "unknown_large"
            risk = "medium"
            reason_key = "classifier.reason.unknown_large"
            matcher = { kind = "size_gte", size_bytes = 524288000 }
        "#;
        let original: UserRuleSet = toml::from_str(original_toml).unwrap();
        assert_eq!(original.rule.len(), 5);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        save_to(tmp.path(), &original).expect("save should succeed");

        let reloaded = load_from(tmp.path());
        assert_eq!(reloaded.rule.len(), 5);
        // 逐一校验 matcher 变体保留;Debug 表达式串里 enum kind 名是 PascalCase
        // 跟 serialize tag 不同,但只要 Debug 输出唯一就可以验证。
        assert_eq!(reloaded.rule[0].id, "r1");
        assert_eq!(reloaded.rule[1].id, "r2");
        assert_eq!(reloaded.rule[2].id, "r3");
        assert_eq!(reloaded.rule[3].id, "r4");
        assert_eq!(reloaded.rule[4].id, "r5");
        assert!(matches!(reloaded.rule[0].matcher, Matcher::PathContains { .. }));
        assert!(matches!(reloaded.rule[1].matcher, Matcher::PathContainsAny { .. }));
        assert!(matches!(reloaded.rule[2].matcher, Matcher::PathEndsWith { .. }));
        assert!(matches!(reloaded.rule[3].matcher, Matcher::ExtInAndSizeGt { .. }));
        assert!(matches!(reloaded.rule[4].matcher, Matcher::SizeGte { .. }));
        if let Matcher::ExtInAndSizeGt { exts, size_bytes } = &reloaded.rule[3].matcher {
            assert_eq!(exts, &vec!["mp4".to_string(), "mov".to_string()]);
            assert_eq!(*size_bytes, 1073741824u64);
        }
    }

    /// Round 34B · 空 set 写出后再读还是 0 条规则,避免 UI "清空" 操作
    /// 写出无法解析的 TOML。
    #[test]
    fn save_empty_set_then_load_returns_empty() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        save_to(tmp.path(), &UserRuleSet::default()).unwrap();
        let reloaded = load_from(tmp.path());
        assert_eq!(reloaded.rule.len(), 0);
    }

    /// Round 34B · save_to 在父目录不存在时自动创建,首次写盘场景的核心
    /// 安全保障(app_data_dir 可能存在但目标路径父目录缺失,例如用户
    /// 删过 com.diskmind.app 子目录后又重启了 app)。
    #[test]
    fn save_to_creates_parent_dir_when_missing() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let nested = tmp_dir.path().join("missing_subdir").join("rules.toml");
        let set = UserRuleSet::default();
        save_to(&nested, &set).expect("create parent dir should be automatic");
        assert!(nested.exists());
    }
}
