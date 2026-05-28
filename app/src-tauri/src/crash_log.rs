//! 崩溃 / 异常本地日志(M5 错误监控,Sprint 2 · S6 + S7 共享)
//!
//! 设计目标:
//! - **Rust panic** 经 `std::panic::set_hook` 写到 `<app_data>/logs/crash.log`,
//!   `Backtrace::capture()` 抓栈;不阻塞主线程,出错只往 stderr 退化。
//! - **前端异常**(`onErrorCaptured` / `window error` / `unhandledrejection`)
//!   走 IPC `log_frontend_error` 写**同一份**日志,统一查阅口径。
//! - 每条记录是一行 JSON(JSONL 友好,直接 `tail` / `jq` 可读);按"行数"
//!   而不是"字节数 / 天数"做封顶 — 简单可证 + 不依赖系统时间。
//! - panic_hook 是全局 hook(没有 AppHandle),日志目录通过 `OnceLock` 在
//!   `setup` 里注入;hook 触发时若目录还没设(极早期 panic),退化为 stderr。
//!
//! 不做的事:
//! - 不上报第三方(Sentry / 自建 endpoint)— Sprint 3 S13 再决策。
//! - 不做按天分割文件 — alpha 阶段 1000 行容量足够,后续真有需要再加。
//! - 不在 panic_hook 里走 tokio / async — 避免 panic 嵌套。

use std::backtrace::Backtrace;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// 单个文件保留的最大行数。超过后裁掉最老的 N 条,保证文件不无限增长。
/// 1000 行 × ~500 字节 ≈ 500 KB,人工 review 完全够用。
const MAX_LOG_LINES: usize = 1000;

/// 触发裁剪的阈值:超过 `MAX_LOG_LINES * 1.2` 才裁剪一次(避免每次写都裁,
/// IO 抖动)。
const TRIM_THRESHOLD_LINES: usize = MAX_LOG_LINES + MAX_LOG_LINES / 5;

static CRASH_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 写文件时持的锁。panic_hook 是任意线程触发,前端 IPC 也是线程池,需要
/// 串行化 append + trim,否则可能写到一半被截断。
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrashEntry {
    /// 毫秒级 UNIX 时间戳(写入时刻)。前端可直接 `new Date(ts).toLocaleString()`。
    pub ts: i64,
    /// 大小写敏感的级别字符串。当前实际只有 `panic` / `error` / `warn`,
    /// 但为了让前端透传无损,这里收个 String 不做 enum。
    pub level: String,
    /// 来源标识:`rust:panic` / `frontend:onErrorCaptured` / `frontend:window-error`
    /// / `frontend:unhandledrejection`。
    pub source: String,
    /// 单行简短消息(panic message / error.message)。
    pub message: String,
    /// 多行 backtrace / stack trace。无栈时可为空。
    pub stack: String,
}

/// 由 `setup` 在拿到 `app_data_dir` 之后调用一次。重复调用会被忽略
/// (`OnceLock` 语义)。`dir` 不存在时不在这里创建 — 调用方负责
/// `create_dir_all`。
pub fn init_dir(dir: PathBuf) {
    let _ = CRASH_LOG_DIR.set(dir);
}

/// 返回崩溃日志所在目录。`init_dir` 还没跑时返回 None。
pub fn dir() -> Option<&'static Path> {
    CRASH_LOG_DIR.get().map(|p| p.as_path())
}

/// 返回崩溃日志文件路径。
fn log_path() -> Option<PathBuf> {
    CRASH_LOG_DIR.get().map(|d| d.join("crash.log"))
}

/// 安装 Rust panic_hook。**必须**在 `tauri::Builder::default()` 之前调用,
/// 否则 builder 内部的早期 panic 抓不到。
///
/// hook 内部刻意不依赖任何 Tauri / tokio / async 设施 —— 出现"panic 嵌套
/// 进 panic_hook"会让进程直接 abort 而无任何日志,完全相反的体验。
pub fn install_panic_hook() {
    // 保留默认 hook(往 stderr 打印),我们在它后面追加文件写入。
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // 先让默认 hook 把 panic 打到 stderr — 这样即使下面写文件失败,
        // 用户也能在 `pnpm tauri:dev` 终端看到 panic。
        default(info);

        let message = panic_message(info);
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let backtrace = Backtrace::capture().to_string();
        let entry = CrashEntry {
            ts: now_ms(),
            level: "panic".to_string(),
            source: format!("rust:panic@{location}"),
            message,
            stack: backtrace,
        };
        // 不传播写入错误 — panic_hook 失败只能往 stderr 报,不能再 panic。
        if let Err(e) = append_entry_internal(&entry) {
            eprintln!("[diskmind] crash_log write failed during panic: {e}");
        }
    }));
}

/// 前端 / Rust 外部调用方写一条日志记录。**Rust panic 走 `install_panic_hook`
/// 自动写入**,不需要业务代码再调本函数。
pub fn append(level: &str, source: &str, message: &str, stack: &str) -> Result<(), String> {
    let entry = CrashEntry {
        ts: now_ms(),
        level: level.to_string(),
        source: source.to_string(),
        message: message.to_string(),
        stack: stack.to_string(),
    };
    append_entry_internal(&entry).map_err(|e| e.to_string())
}

/// 读取最近 N 条记录(按时间倒序返回 — 最新的在前)。`n=0` 返回全部。
pub fn read_recent(n: usize) -> Result<Vec<CrashEntry>, String> {
    let path = match log_path() {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut entries: Vec<CrashEntry> = reader
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<CrashEntry>(&line).ok())
        .collect();
    entries.reverse();
    if n > 0 && entries.len() > n {
        entries.truncate(n);
    }
    Ok(entries)
}

fn append_entry_internal(entry: &CrashEntry) -> std::io::Result<()> {
    let path = match log_path() {
        Some(p) => p,
        None => {
            // 极早期 panic(setup 之前),目录还没注入 — 只能 stderr。
            eprintln!(
                "[diskmind] crash_log not initialized; entry dropped: level={} source={} msg={}",
                entry.level, entry.source, entry.message
            );
            return Ok(());
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(entry).unwrap_or_else(|_| "{}".to_string());

    let _g = WRITE_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut file = OpenOptions::new().append(true).create(true).open(&path)?;
    writeln!(file, "{line}")?;
    file.flush()?;
    drop(file);

    maybe_trim(&path)?;
    Ok(())
}

/// 按 line count 裁剪。仅当行数 ≥ TRIM_THRESHOLD 时一次性裁掉最老的部分,
/// 让文件维持在 MAX_LOG_LINES 附近。
fn maybe_trim(path: &Path) -> std::io::Result<()> {
    let file = File::open(path)?;
    let line_count = BufReader::new(file).lines().count();
    if line_count < TRIM_THRESHOLD_LINES {
        return Ok(());
    }

    let drop_n = line_count - MAX_LOG_LINES;
    let file = File::open(path)?;
    let tail: Vec<String> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .skip(drop_n)
        .collect();

    let tmp = path.with_extension("log.tmp");
    {
        let mut out = File::create(&tmp)?;
        for line in &tail {
            writeln!(out, "{line}")?;
        }
        out.flush()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    // `&'static str` 和 `String` 是 panic! 最常见的 payload 类型,先各试一遍。
    if let Some(s) = info.payload().downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = info.payload().downcast_ref::<String>() {
        return s.clone();
    }
    // payload 是其他类型(例如自定义 enum)— 退化到 Debug 表示。
    format!("{info}")
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
