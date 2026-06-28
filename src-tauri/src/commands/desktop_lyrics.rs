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

/// 窗口不透明度范围与默认值（0.0 全透明，1.0 不透明）
pub const OPACITY_MIN: f32 = 0.1;
pub const OPACITY_MAX: f32 = 1.0;
pub const OPACITY_DEFAULT: f32 = 1.0;

/// serde 反序列化默认值：旧版配置文件缺少 `opacity` 字段时使用默认值，
/// 避免整个配置解析失败导致所有设置被重置。
fn default_opacity() -> f32 { OPACITY_DEFAULT }

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
    /// 卡拉OK逐字播放模式（开启后逐字高亮，关闭则整行高亮）
    pub karaoke: bool,
    /// 显示行数：1=单行，2=双行
    pub line_count: u32,
    /// 显示方向："horizontal" 或 "vertical"
    pub direction: String,
    /// 窗口不透明度（0.1~1.0），由前端 CSS 应用到桌面歌词容器
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// 桌面歌词窗口是否开启（跨会话记忆状态）。
    /// 用户开启/关闭桌面歌词时由前端写入，应用启动时读取以自动恢复。
    #[serde(default)]
    pub visible: bool,
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
            karaoke: true,
            line_count: 1,
            direction: "horizontal".to_string(),
            opacity: OPACITY_DEFAULT,
            visible: false,
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
    // 显示行数仅允许 1 或 2
    if s.line_count != 1 && s.line_count != 2 {
        s.line_count = 1;
    }
    // 显示方向仅允许 horizontal 或 vertical
    if s.direction != "horizontal" && s.direction != "vertical" {
        s.direction = "horizontal".to_string();
    }
    // 不透明度限制在 [OPACITY_MIN, OPACITY_MAX]，NaN/无穷回退默认
    if s.opacity.is_nan() || s.opacity.is_infinite() {
        s.opacity = OPACITY_DEFAULT;
    } else if s.opacity < OPACITY_MIN {
        s.opacity = OPACITY_MIN;
    } else if s.opacity > OPACITY_MAX {
        s.opacity = OPACITY_MAX;
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
    karaoke: Option<bool>,
    line_count: Option<u32>,
    direction: Option<String>,
    opacity: Option<f32>,
    visible: Option<bool>,
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
    if let Some(lc) = line_count {
        if lc != 1 && lc != 2 {
            return Err("显示行数仅支持 1（单行）或 2（双行）".into());
        }
    }
    if let Some(ref dir) = direction {
        if dir != "horizontal" && dir != "vertical" {
            return Err("显示方向仅支持 horizontal 或 vertical".into());
        }
    }
    if let Some(op) = opacity {
        if op.is_nan() || op.is_infinite() || !(OPACITY_MIN..=OPACITY_MAX).contains(&op) {
            return Err(format!("不透明度超出范围 [{}, {}]", OPACITY_MIN, OPACITY_MAX));
        }
    }

    let mut guard = state.desktop_lyrics.lock();
    if let Some(fs) = font_size { guard.font_size = fs; }
    if let Some(l) = locked { guard.locked = l; }
    if let Some(fam) = font_family { guard.font_family = fam; }
    if let Some(b) = bold { guard.bold = b; }
    if let Some(it) = italic { guard.italic = it; }
    if let Some(col) = font_color { guard.font_color = col.trim().to_string(); }
    if let Some(k) = karaoke { guard.karaoke = k; }
    if let Some(lc) = line_count { guard.line_count = lc; }
    if let Some(dir) = direction { guard.direction = dir; }
    if let Some(op) = opacity { guard.opacity = op; }
    if let Some(v) = visible { guard.visible = v; }
    let current = guard.clone();
    drop(guard);

    if let Err(e) = persist_settings(&current) {
        tracing::warn!("Failed to persist desktop lyrics settings: {}", e);
    }

    let _ = app.emit("desktop-lyrics-settings-changed", &current);
    Ok(current)
}

/// 恢复所有桌面歌词设置到默认值（字号/锁定/字体族/样式/颜色/卡拉OK/显示模式）。
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

// ── 全局鼠标坐标获取（用于锁定状态下轮询检测鼠标是否在角落区域）──

#[cfg(target_os = "windows")]
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn GetCursorPos(lp_point: *mut Point) -> i32;
}

/// 返回鼠标在屏幕上的物理坐标（像素）。
/// 用于桌面歌词锁定后轮询检测：鼠标是否悬停在右上角解锁按钮区域，
/// 以便动态切换 `setIgnoreCursorEvents` 实现穿透 + 可交互的共存。
#[tauri::command]
pub fn get_cursor_position() -> Result<(i32, i32), String> {
    #[cfg(target_os = "windows")]
    {
        let mut pt = Point { x: 0, y: 0 };
        unsafe {
            if GetCursorPos(&mut pt) != 0 {
                return Ok((pt.x, pt.y));
            }
        }
        Err("GetCursorPos failed".into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("get_cursor_position is only supported on Windows".into())
    }
}
