//! 后端 i18n marker 字符串工具(Round 26)。
//!
//! 历史背景:DiskMind 后端早期把所有面向用户的字符串(error / 进度
//! emit / TrashFailure.message / classifier 风险描述)都直接硬编码中
//! 文。前端 vue-i18n 在 zh ↔ en 切换时,这些经 IPC 流过来的中文不会
//! 跟随重渲染 — 用户切到 English 仍看到一半的中文,体验割裂。
//!
//! ## 为什么是 marker 而不是结构化 `UiError { code, params }`
//!
//! 纯架构角度结构化错误码更优(类型安全、JSON 可读、嵌套表达力)。
//! 但 Tauri IPC 里 `Result<T, String>` 是一等公民,要换 `Result<T,
//! UiError>` 全栈接口都要改 — 70 处字面量会膨胀到 200+ 处涉及面。同
//! 时 `TrashFailure { message: String }` 这类嵌套字段也得改 schema。
//!
//! marker 方案:保留 `String` 接口,把字符串内容约定为
//! `"$i18n:<key>|<k=v>,<k=v>"` 格式。前端在 IPC 边界统一调
//! `localize()` 翻译,Rust 端不需要任何类型改造。
//!
//! ## 格式约定
//!
//! - 无参数:`$i18n:scan.no_target`
//! - 带参数:`$i18n:scan.io_error|err=Permission%20denied,path=%2Ftmp`
//!   - 参数值用 `urlencoding::encode` 处理,避免 `=` / `,` / `|`
//!     冲突
//!   - 参数 key 必须是简单 ASCII 标识符,不做转义
//!
//! 所有用户可见 marker 都通过本模块的两个 helper 构造,禁止在业务代
//! 码里手动拼字符串 — 集中点便于将来重构成结构化 `UiError`。

/// 构造无参数 i18n marker。
///
/// 用法:`i18n("scan.no_target")` → `"$i18n:scan.no_target"`。
/// 调用方拿到的 `String` 直接放进 `Err(...)` / 进度 emit 即可,前端
/// `localize()` 会识别 `$i18n:` 前缀并调 `t(key)`。
#[inline]
pub fn i18n(key: &str) -> String {
    format!("$i18n:{}", key)
}

/// 构造带参数 i18n marker。
///
/// 用法:
/// ```ignore
/// i18n_p("scan.io_error", &[("err", &e.to_string())])
/// // → "$i18n:scan.io_error|err=Permission%20denied"
/// ```
///
/// 参数 value 经 percent-encode 处理,前端 `localize()` 解码回原字符
/// 串后传给 `t(key, params)`。
pub fn i18n_p(key: &str, params: &[(&str, &str)]) -> String {
    if params.is_empty() {
        return i18n(key);
    }
    let mut out = String::with_capacity(key.len() + 8 + params.len() * 16);
    out.push_str("$i18n:");
    out.push_str(key);
    out.push('|');
    let mut first = true;
    for (k, v) in params {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(k);
        out.push('=');
        out.push_str(&percent_encode(v));
    }
    out
}

/// 极简 percent-encode — 只转义会破坏 marker 格式的字符:
/// `=` / `,` / `|` / `%` / 空格 + 任何非 ASCII。其他保留以提升日志
/// 可读性(路径里的 `/` 不转义)。
///
/// 不引第三方依赖(`urlencoding` / `percent-encoding`)是为了保持
/// `i18n` 模块零依赖,避免在编译图中再添一个 crate。前端 `localize`
/// 用 `decodeURIComponent` 反向兼容(`decodeURIComponent` 不要求严格
/// RFC 3986,容忍部分未转义字符)。
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        let needs_escape = matches!(ch, '=' | ',' | '|' | '%' | ' ')
            || !ch.is_ascii()
            || ch.is_ascii_control();
        if needs_escape {
            for b in ch.to_string().as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_no_params() {
        assert_eq!(i18n("scan.no_target"), "$i18n:scan.no_target");
    }

    #[test]
    fn marker_with_single_param() {
        assert_eq!(
            i18n_p("scan.io_error", &[("err", "Permission denied")]),
            "$i18n:scan.io_error|err=Permission%20denied"
        );
    }

    #[test]
    fn marker_with_multiple_params() {
        let m = i18n_p(
            "ai_classify.batch_progress",
            &[("batch", "3"), ("total", "8")],
        );
        assert_eq!(m, "$i18n:ai_classify.batch_progress|batch=3,total=8");
    }

    #[test]
    fn marker_escapes_special_chars() {
        // `,` `=` `|` 都会破坏 marker 格式,必须转义
        let m = i18n_p("trash.error", &[("err", "key=value, with|pipe")]);
        assert_eq!(
            m,
            "$i18n:trash.error|err=key%3Dvalue%2C%20with%7Cpipe"
        );
    }

    #[test]
    fn marker_handles_unicode() {
        // 中文字符也能塞进 marker(percent-encoded),前端 decodeURIComponent
        // 能还原
        let m = i18n_p("test", &[("name", "测试")]);
        assert!(m.contains("name=%E6%B5%8B%E8%AF%95"));
    }

    #[test]
    fn marker_empty_params_falls_back_to_keyless() {
        // 空 params 切片不能产出尾部 `|`,否则前端解析会拿到空字符串 key
        assert_eq!(i18n_p("scan.no_target", &[]), "$i18n:scan.no_target");
    }
}
