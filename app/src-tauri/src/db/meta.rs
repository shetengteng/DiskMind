//! key-value `meta` 表的读写。当前两类配置:
//! - `trash_retention_days` — 沙箱保留天数(用户可改 7/14/30/60)
//! - `max_scan_history`     — 扫描历史保留次数(用户可改 10-200)
//!
//! 读取时若 key 不存在或解析失败,回退到调用方传入的默认值,避免 db 层
//! 硬编码业务常量。

use rusqlite::params;

use super::{Db, MAX_SCAN_HISTORY_MAX, MAX_SCAN_HISTORY_MIN};

impl Db {
    /// 沙箱保留天数,持久化在 `meta` 表的 `trash_retention_days` 键里。
    /// 不存在或解析失败时回退到 `default_days`,让调用方决定默认值
    /// (避免 db 层硬编码业务常量)。
    pub fn trash_retention_days(&self, default_days: u64) -> u64 {
        let conn = self.conn.lock().expect("db poisoned");
        conn.query_row(
            "SELECT v FROM meta WHERE k = 'trash_retention_days'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(default_days)
    }

    pub fn set_trash_retention_days(&self, days: u64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO meta(k, v) VALUES('trash_retention_days', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![days.to_string()],
        )?;
        Ok(())
    }

    /// 扫描历史保留上限,持久化在 `meta` 表的 `max_scan_history` 键里。
    /// 不存在或解析失败时回退到 `default_value`(与 `trash_retention_days`
    /// 同模式,避免 db 层硬编码业务常量)。
    pub fn max_scan_history(&self, default_value: i64) -> i64 {
        let conn = self.conn.lock().expect("db poisoned");
        super::read_max_scan_history(&conn, default_value)
    }

    pub fn set_max_scan_history(&self, value: i64) -> rusqlite::Result<()> {
        let clamped = value.clamp(MAX_SCAN_HISTORY_MIN, MAX_SCAN_HISTORY_MAX);
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO meta(k, v) VALUES('max_scan_history', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![clamped.to_string()],
        )?;
        Ok(())
    }

    /// 「关闭窗口时最小化到托盘」开关的持久化状态(S12 · M5 收官)。
    /// 默认 false:向后兼容,Windows 用户点 X 仍然退出应用,macOS 维持
    /// 默认 hide-on-close 系统行为。开启后:任何平台点 X / Cmd+W 都
    /// 走 hide window 不退出,Tray icon 始终保持可见,用户可从 tray 重
    /// 新唤出或显式 Quit。
    pub fn hide_in_tray_when_minimized(&self, default_value: bool) -> bool {
        let conn = self.conn.lock().expect("db poisoned");
        conn.query_row(
            "SELECT v FROM meta WHERE k = 'hide_in_tray_when_minimized'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(default_value)
    }

    pub fn set_hide_in_tray_when_minimized(&self, value: bool) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO meta(k, v) VALUES('hide_in_tray_when_minimized', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![if value { "1" } else { "0" }],
        )?;
        Ok(())
    }

    /// 上次用户在崩溃报告 dialog 中 dismiss 的最新 panic 时间戳(毫秒)。
    /// S13 · 启动时只展示 ts > 此值的 panic 记录,避免每次开 app 都重复
    /// 弹窗。默认 0(从未 dismiss 过)。
    pub fn last_seen_crash_ts(&self, default_value: i64) -> i64 {
        let conn = self.conn.lock().expect("db poisoned");
        conn.query_row(
            "SELECT v FROM meta WHERE k = 'last_seen_crash_ts'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(default_value)
    }

    pub fn set_last_seen_crash_ts(&self, ts: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO meta(k, v) VALUES('last_seen_crash_ts', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![ts.to_string()],
        )?;
        Ok(())
    }
}
