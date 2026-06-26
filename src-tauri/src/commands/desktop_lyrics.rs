use std::sync::Arc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use crate::state::AppState;

/// 桌面歌词字号范围（像素）
pub const FONT_MIN: u32 = 12;
pub const FONT_MAX: u32 = 48;
pub const FONT_DEFAULT: u32 = 28;

/// 默认字体族（与原实现一致，回退到 system-ui）
pub const FONT_FAMILY_DEFAULT: &str = "system-ui, sans-serif";
/// 默认字体颜色（主题强调色，与原歌词渲染一致）
pub const FONT_COLOR_DEFAULT: &str = "#a78bfa";

/// 桌面歌词运行时设置：字号 + 窗口位置锁定 + 字体族/样式/颜色。
///
/// 通过简单的 JSON 配置文件持久化，重启后保留。设置变更时后端通过
/// `desktop-lyrics-settings-changed` 事件广播给所有窗口，主窗口的设置
/// 面板与桌面歌词窗口据此双向同步。
///
/// `font_family` / `font_color` 为字符串，由前端选择与校验；后端仅做
/// 粗粒度长度限制以避免配置文件被恶意写入超长值。
#[derive(Clone, Serialize, Deserialize)]
pub struct DesktopLyricsSettings {
    pub font_size: u32,
    pub locked: bool,
    pub font_family: String,
    /// 粗体
    pub bold: bool,
    /// 斜体
    pub italic: bool,
    /// 字体颜色（#RRGGBB）
    pub font_color: String,
}

impl Default for DesktopLyricsSettings {
    fn default() -> Self {
        Self {
            font_size: FONT_DEFAULT,
            locked: false,
            font_family: FONT_FAMILY_DEFAULT.to_string(),
            bold: true,
            italic: false,
            font_color: FONT_COLOR_DEFAULT.to_string(),
        }
    }
}

/// 防御性校验：限制字符串长度、规范化字号、校验颜色格式。
fn sanitize(mut s: DesktopLyricsSettings) -> DesktopLyricsSettings {
    if !(FONT_MIN..=FONT_MAX).contains(&s.font_size) {
        s.font_size = FONT_DEFAULT;
    }
    if s.font_family.chars().count() > 256 {
        s.font_family = FONT_FAMILY_DEFAULT.to_string();
    }
    // 颜色必须形如 #RRGGBB
    let col = s.font_color.trim();
    let valid = col.len() == 7
        && col.starts_with('#')
        && col[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        s.font_color = FONT_COLOR_DEFAULT.to_string();
    } else {
        s.font_color = col.to_string();
    }
    s
}

#[tauri::command]
pub fn desktop_lyrics_get(state: State<'_, AppState>) -> DesktopLyricsSettings {
    state.desktop_lyrics.lock().clone()
}

/// 更新桌面歌词设置。所有字段均为 `Option`，`None` 表示保持原值。
/// 变更后向所有窗口广播 `desktop-lyrics-settings-changed` 事件。
#[tauri::command]
pub fn desktop_lyrics_set(
    app: AppHandle,
    state: State<'_, AppState>,
    font_size: Option<u32>,
    locked: Option<bool>,
    font_family: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    font_color: Option<String>,
) -> Result<DesktopLyricsSettings, String> {
    if let Some(fs) = font_size {
        if !(FONT_MIN..=FONT_MAX).contains(&fs) {
            return Err(format!("字号超出范围 [{}, {}]", FONT_MIN, FONT_MAX));
        }
    }
    if let Some(ref fam) = font_family {
        if fam.chars().count() > 256 {
            return Err("字体族名称过长".into());
        }
    }
    if let Some(ref col) = font_color {
        let c = col.trim();
        let valid = c.len() == 7
            && c.starts_with('#')
            && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err("颜色格式无效，需为 #RRGGBB".into());
        }
    }

    let mut guard = state.desktop_lyrics.lock();
    if let Some(fs) = font_size { guard.font_size = fs; }
    if let Some(l) = locked { guard.locked = l; }
    if let Some(fam) = font_family { guard.font_family = fam; }
    if let Some(b) = bold { guard.bold = b; }
    if let Some(it) = italic { guard.italic = it; }
    if let Some(col) = font_color { guard.font_color = col.trim().to_string(); }
    let current = guard.clone();
    drop(guard);

    if let Err(e) = persist_settings(&current) {
        tracing::warn!("Failed to persist desktop lyrics settings: {}", e);
    }

    let _ = app.emit("desktop-lyrics-settings-changed", &current);
    Ok(current)
}

/// 恢复所有桌面歌词设置到默认值（字号/锁定/字体族/样式/颜色）。
#[tauri::command]
pub fn desktop_lyrics_reset(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DesktopLyricsSettings, String> {
    let default = DesktopLyricsSettings::default();
    {
        let mut guard = state.desktop_lyrics.lock();
        *guard = default.clone();
    }
    if let Err(e) = persist_settings(&default) {
        tracing::warn!("Failed to persist desktop lyrics settings: {}", e);
    }
    let _ = app.emit("desktop-lyrics-settings-changed", &default);
    Ok(default)
}

/// 启动时加载持久化的桌面歌词设置（带 sanitize）。
pub fn load_settings() -> Arc<Mutex<DesktopLyricsSettings>> {
    let loaded = match std::fs::read_to_string(config_path()) {
        Ok(content) => serde_json::from_str::<DesktopLyricsSettings>(&content)
            .map(sanitize)
            .unwrap_or_default(),
        Err(_) => DesktopLyricsSettings::default(),
    };
    Arc::new(Mutex::new(loaded))
}

fn persist_settings(settings: &DesktopLyricsSettings) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)
}

fn config_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ttplayer-next")
        .join("desktop_lyrics_settings.json")
}
