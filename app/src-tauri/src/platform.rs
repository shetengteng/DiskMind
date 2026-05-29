//! 平台 / 磁盘信息 + 系统文件管理器拉起。这些命令不依赖 ScanState,
//! 但因为接口形状是 Tauri 命令,所以单独成文件而非塞回 commands/。
//!
//! - `disk_usage` / `disk_usage_for`: 返回某个挂载点的容量统计,Dashboard
//!   / DiskOverviewCard 等组件直接消费。
//! - `reveal_in_explorer`: macOS Finder / Windows Explorer / Linux xdg-open。
//! - `platform_info`: 平台元数据 + 推荐扫描根目录,首次启动 Onboarding 用。

use std::path::PathBuf;

use serde::Serialize;

use crate::state::expand_root;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsageInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
    pub mount_point: String,
    pub name: String,
}

#[tauri::command]
pub fn disk_usage() -> Result<DiskUsageInfo, String> {
    pick_disk_for(None)
}

#[tauri::command]
pub fn disk_usage_for(path: String) -> Result<DiskUsageInfo, String> {
    pick_disk_for(Some(path))
}

fn pick_disk_for(path: Option<String>) -> Result<DiskUsageInfo, String> {
    use sysinfo::Disks;

    let disks = Disks::new_with_refreshed_list();
    if disks.is_empty() {
        return Err("no disk found".to_string());
    }

    let resolved_path = path
        .as_deref()
        .and_then(expand_root)
        .and_then(|p| std::fs::canonicalize(&p).ok().or(Some(p)));

    let chosen = if let Some(target) = resolved_path {
        // 与 mount_point 做最长前缀匹配
        let mut best: Option<&sysinfo::Disk> = None;
        let mut best_len: usize = 0;
        for d in disks.iter() {
            let mp = d.mount_point();
            if target.starts_with(mp) {
                let len = mp.as_os_str().len();
                if best.is_none() || len > best_len {
                    best = Some(d);
                    best_len = len;
                }
            }
        }
        best.or_else(|| disks.iter().max_by_key(|d| d.total_space()))
    } else {
        disks
            .iter()
            .filter(|d| d.mount_point() == std::path::Path::new("/"))
            .max_by_key(|d| d.total_space())
            .or_else(|| disks.iter().max_by_key(|d| d.total_space()))
    };

    let d = chosen.ok_or_else(|| "no disk found".to_string())?;
    let total = d.total_space();
    let available = d.available_space();
    let used = total.saturating_sub(available);
    let used_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    Ok(DiskUsageInfo {
        total_bytes: total,
        available_bytes: available,
        used_bytes: used,
        used_percent,
        mount_point: d.mount_point().to_string_lossy().to_string(),
        name: d.name().to_string_lossy().to_string(),
    })
}

/// 跨平台地在系统文件管理器里展示 `path`(macOS Finder / Windows Explorer
/// / Linux 的 xdg-open)。对目录就打开它,对文件就高亮显示。
/// 不存在或调用失败时返回 `Err(String)`,由前端 toast。
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(crate::i18n::i18n_p(
            "platform.error.path_not_found",
            &[("path", &path)],
        ));
    }

    #[cfg(target_os = "macos")]
    {
        // `open -R` 在 Finder 里高亮显示目标;对目录则会打开它的父目录
        // 并高亮,所以这里也作为通用入口使用。
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // `explorer /select,<path>` 高亮显示;对目录改用 `explorer <path>`
        // 直接打开,避免 explorer 在选中目录时弹出二级窗口。
        let mut cmd = std::process::Command::new("explorer");
        if p.is_dir() {
            cmd.arg(&path);
        } else {
            cmd.arg(format!("/select,{}", path));
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // freedesktop 没有"高亮文件"通用入口,退化为打开父目录。
        let target = if p.is_dir() {
            p.to_path_buf()
        } else {
            p.parent().unwrap_or(p).to_path_buf()
        };
        std::process::Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// 按平台返回推荐扫描路径。返回值包含当前 OS,以及一组**可能存在**且
/// 值得默认扫描的路径。每个候选路径都会基于 `dirs::home_dir()` 做规范化
/// 并校验是否真实存在,确保首次启动时在 Windows / Linux / macOS 任意
/// locale 下都不会出现“失效路径”。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    /// 取值: "macos" | "windows" | "linux" | "unknown"
    os: &'static str,
    /// 路径分隔符 (`"/"` 或 `"\\"`)。
    sep: &'static str,
    /// 已经做过存在性校验的推荐扫描目标。每条记录包含平台原生分隔符的
    /// 绝对路径字符串,以及一个便于展示的简短 label。
    suggested_targets: Vec<SuggestedTarget>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedTarget {
    path: String,
    /// 供前端做 i18n 映射用的稳定标识 (home / downloads / documents /
    /// desktop / applications / appdata / temp),前端据此查表得到本地化
    /// 显示文案。
    kind: &'static str,
}

#[tauri::command]
pub fn platform_info() -> PlatformInfo {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let sep = if cfg!(target_os = "windows") {
        "\\"
    } else {
        "/"
    };

    let mut suggested: Vec<SuggestedTarget> = Vec::new();
    let mut push = |kind: &'static str, p: Option<PathBuf>| {
        if let Some(p) = p {
            if p.exists() {
                suggested.push(SuggestedTarget {
                    path: p.to_string_lossy().into_owned(),
                    kind,
                });
            }
        }
    };

    push("home", dirs::home_dir());
    push("downloads", dirs::download_dir());
    push("documents", dirs::document_dir());
    push("desktop", dirs::desktop_dir());
    push("pictures", dirs::picture_dir());
    push("videos", dirs::video_dir());

    #[cfg(target_os = "macos")]
    {
        let apps = PathBuf::from("/Applications");
        if apps.exists() {
            suggested.push(SuggestedTarget {
                path: apps.to_string_lossy().into_owned(),
                kind: "applications",
            });
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 上 AppData 是清理候选的高价值目录。
        if let Some(local) = dirs::data_local_dir() {
            if local.exists() {
                suggested.push(SuggestedTarget {
                    path: local.to_string_lossy().into_owned(),
                    kind: "appdata",
                });
            }
        }
    }

    PlatformInfo {
        os,
        sep,
        suggested_targets: suggested,
    }
}
