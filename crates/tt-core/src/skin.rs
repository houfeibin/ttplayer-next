use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Skin metadata from skin.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinMeta {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub preview: Option<String>,
}

/// Color definitions for a skin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinColors {
    #[serde(default = "default_bg_primary")]
    pub bg_primary: String,
    #[serde(default = "default_bg_secondary")]
    pub bg_secondary: String,
    #[serde(default = "default_bg_tertiary")]
    pub bg_tertiary: String,
    #[serde(default = "default_text_primary")]
    pub text_primary: String,
    #[serde(default = "default_text_secondary")]
    pub text_secondary: String,
    #[serde(default = "default_highlight")]
    pub highlight: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_accent_hover")]
    pub accent_hover: String,
    /// A light tint of the accent, used for active/hover text & highlights.
    /// If absent, it is computed by lightening `accent` toward white.
    #[serde(default)]
    pub accent_light: Option<String>,
    #[serde(default = "default_progress_fill")]
    pub progress_fill: String,
    #[serde(default = "default_progress_bg")]
    pub progress_bg: String,
    #[serde(default = "default_border_color")]
    pub border_color: String,
    #[serde(default = "default_panel_bg")]
    pub panel_bg: String,
    #[serde(default = "default_button_bg")]
    pub button_bg: String,
    #[serde(default = "default_button_hover")]
    pub button_hover: String,
    #[serde(default)]
    pub spectrum_top: Option<String>,
    #[serde(default)]
    pub spectrum_bottom: Option<String>,
    #[serde(default)]
    pub spectrum_peak: Option<String>,
}

fn default_bg_primary() -> String { "#0f061a".into() }
fn default_bg_secondary() -> String { "#1a0f2e".into() }
fn default_bg_tertiary() -> String { "#251540".into() }
fn default_text_primary() -> String { "#e0d4ff".into() }
fn default_text_secondary() -> String { "#a78bfa".into() }
fn default_highlight() -> String { "#f5d78a".into() }
fn default_accent() -> String { "#7c3aed".into() }
fn default_accent_hover() -> String { "#8b5cf6".into() }
fn default_progress_fill() -> String { "#8b5cf6".into() }
fn default_progress_bg() -> String { "#2a1a4a".into() }
fn default_border_color() -> String { "#3a2a5a".into() }
fn default_panel_bg() -> String { "#0f061a".into() }
fn default_button_bg() -> String { "#2a1a4a".into() }
fn default_button_hover() -> String { "#3a2a5a".into() }

/// Parse a `#RGB` or `#RRGGBB` hex string into `(r, g, b)`.
/// Returns `None` for malformed input.
fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        3 => Some((
            u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?,
            u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?,
            u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?,
        )),
        6 => Some((
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
        )),
        _ => None,
    }
}

/// Produce a lightened hex color by mixing `(r, g, b)` toward white by `amount` (0..1).
fn lighten_hex((r, g, b): (u8, u8, u8), amount: f32) -> String {
    let lr = (r as f32 + (255.0 - r as f32) * amount).round() as u8;
    let lg = (g as f32 + (255.0 - g as f32) * amount).round() as u8;
    let lb = (b as f32 + (255.0 - b as f32) * amount).round() as u8;
    format!("#{:02x}{:02x}{:02x}", lr, lg, lb)
}

impl Default for SkinColors {
    fn default() -> Self {
        Self {
            bg_primary: default_bg_primary(),
            bg_secondary: default_bg_secondary(),
            bg_tertiary: default_bg_tertiary(),
            text_primary: default_text_primary(),
            text_secondary: default_text_secondary(),
            highlight: default_highlight(),
            accent: default_accent(),
            accent_hover: default_accent_hover(),
            progress_fill: default_progress_fill(),
            progress_bg: default_progress_bg(),
            border_color: default_border_color(),
            panel_bg: default_panel_bg(),
            button_bg: default_button_bg(),
            button_hover: default_button_hover(),
            spectrum_top: None,
            spectrum_bottom: None,
            spectrum_peak: None,
            accent_light: None,
        }
    }
}

/// Background image for a UI region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinBackground {
    /// Image file path (relative to skin dir)
    pub image: String,
    /// CSS background-size: cover | contain | auto | 100% 100%
    #[serde(default = "default_bg_size")]
    pub size: String,
    /// CSS background-position
    #[serde(default = "default_bg_position")]
    pub position: String,
    /// CSS background-repeat
    #[serde(default = "default_bg_repeat")]
    pub repeat: String,
}

fn default_bg_size() -> String { "cover".into() }
fn default_bg_position() -> String { "center".into() }
fn default_bg_repeat() -> String { "no-repeat".into() }

/// Button image set (normal, hover, pressed, disabled states)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonImageSet {
    /// Normal state image
    pub normal: String,
    /// Hover state image (optional, falls back to normal)
    #[serde(default)]
    pub hover: Option<String>,
    /// Pressed state image (optional)
    #[serde(default)]
    pub pressed: Option<String>,
    /// Disabled state image (optional)
    #[serde(default)]
    pub disabled: Option<String>,
}

/// Layout parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinLayout {
    #[serde(default = "default_player_width")]
    pub player_width: u32,
    #[serde(default = "default_player_height")]
    pub player_height: u32,
    #[serde(default = "default_border_radius")]
    pub border_radius: u32,
}

fn default_player_width() -> u32 { 327 }
fn default_player_height() -> u32 { 186 }
fn default_border_radius() -> u32 { 4 }

impl Default for SkinLayout {
    fn default() -> Self {
        Self {
            player_width: default_player_width(),
            player_height: default_player_height(),
            border_radius: default_border_radius(),
        }
    }
}

/// Font configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinFonts {
    #[serde(default = "default_font_title")]
    pub title: String,
    #[serde(default = "default_font_body")]
    pub body: String,
    #[serde(default)]
    pub mono: Option<String>,
}

fn default_font_title() -> String { "'Segoe UI', 'Microsoft YaHei', sans-serif".into() }
fn default_font_body() -> String { "'Segoe UI', 'Microsoft YaHei', sans-serif".into() }

impl Default for SkinFonts {
    fn default() -> Self {
        Self {
            title: default_font_title(),
            body: default_font_body(),
            mono: None,
        }
    }
}

/// Complete skin definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinDefinition {
    #[serde(flatten)]
    pub meta: SkinMeta,
    #[serde(default)]
    pub colors: SkinColors,
    #[serde(default)]
    pub layout: SkinLayout,
    #[serde(default)]
    pub fonts: SkinFonts,
    /// Background images for UI regions
    #[serde(default)]
    pub backgrounds: HashMap<String, SkinBackground>,
    /// Button/state images
    #[serde(default)]
    pub images: HashMap<String, ButtonImageSet>,
    /// Optional custom CSS
    #[serde(default)]
    pub custom_css: Option<String>,
}

/// Skin info for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub is_builtin: bool,
    /// Whether this skin has image assets
    pub has_images: bool,
}

/// Skin manager — loads, stores, and applies skins
pub struct SkinManager {
    current: SkinDefinition,
    current_id: String,
    builtin_skins: HashMap<String, SkinDefinition>,
    skin_dir: PathBuf,
}

impl SkinManager {
    pub fn new() -> Self {
        let builtin = Self::builtin_skins();

        let skin_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ttplayer-next")
            .join("skins");

        Self {
            current: builtin["default"].clone(),
            current_id: "default".into(),
            builtin_skins: builtin,
            skin_dir,
        }
    }

    /// Load built-in skin definitions from individual embedded JSON files.
    ///
    /// Each skin lives in its own `data/builtin-skins/<id>/skin.json` file,
    /// compiled into the binary via `include_str!`. This makes it easy to
    /// edit, add, or remove a built-in skin by managing a single file.
    fn builtin_skins() -> HashMap<String, SkinDefinition> {
        const DEFAULT_JSON: &str = include_str!("data/builtin-skins/default/skin.json");
        const TTPLAYER_BLUE_JSON: &str = include_str!("data/builtin-skins/ttplayer-blue/skin.json");
        const GREEN_JSON: &str = include_str!("data/builtin-skins/green/skin.json");
        const ROSE_JSON: &str = include_str!("data/builtin-skins/rose/skin.json");

        let mut map = HashMap::new();
        for (id, json) in [
            ("default", DEFAULT_JSON),
            ("ttplayer-blue", TTPLAYER_BLUE_JSON),
            ("green", GREEN_JSON),
            ("rose", ROSE_JSON),
        ] {
            match serde_json::from_str::<SkinDefinition>(json) {
                Ok(def) => { map.insert(id.into(), def); }
                Err(e) => {
                    eprintln!("[Skin] Failed to parse builtin skin '{}': {}", id, e);
                }
            }
        }

        // Ultimate fallback: if somehow none parsed, insert a minimal default.
        if !map.contains_key("default") {
            map.insert("default".into(), SkinDefinition {
                meta: SkinMeta {
                    name: "紫韵经典".into(),
                    version: "1.0".into(),
                    author: "TTPlayer".into(),
                    description: "默认紫色主题".into(),
                    preview: None,
                },
                colors: SkinColors::default(),
                layout: SkinLayout::default(),
                fonts: SkinFonts::default(),
                backgrounds: HashMap::new(),
                images: HashMap::new(),
                custom_css: None,
            });
        }
        map
    }

    /// Returns the runtime skins directory path (where skin folders live).
    pub fn skin_dir_path(&self) -> &Path {
        &self.skin_dir
    }

    /// Ensure every built-in skin exists on disk (creates `skin_dir` if needed).
    ///
    /// Missing skins are re-seeded from the compiled-in definitions, while
    /// skins that already exist on disk are **never** overwritten — so user
    /// edits to `skin.json` are always preserved.
    pub fn ensure_skins_on_disk(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.skin_dir)?;
        for (id, skin) in &self.builtin_skins {
            let skin_json = self.skin_dir.join(id).join("skin.json");
            if !skin_json.exists() {
                self.write_skin_to_disk(id, skin)?;
            }
        }
        Ok(())
    }

    /// Write a single skin definition as `skin.json` inside `<skin_dir>/<id>/`.
    fn write_skin_to_disk(&self, id: &str, skin: &SkinDefinition) -> std::io::Result<()> {
        let dir = self.skin_dir.join(id);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(skin)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(dir.join("skin.json"), json)?;
        Ok(())
    }

    /// Delete a skin from disk by ID.
    ///
    /// Returns an error if the skin is `default` (cannot be deleted) or if
    /// the directory doesn't exist.
    pub fn delete_skin(&self, skin_id: &str) -> anyhow::Result<()> {
        if skin_id == "default" {
            anyhow::bail!("默认皮肤不可删除");
        }
        let dir = self.skin_dir.join(skin_id);
        if !dir.exists() {
            anyhow::bail!("皮肤不存在: {}", skin_id);
        }
        std::fs::remove_dir_all(&dir)
            .map_err(|e| anyhow::anyhow!("删除皮肤失败: {}", e))?;
        Ok(())
    }

    /// List all available skins
    pub fn list_skins(&self) -> Vec<SkinInfo> {
        let mut skins: Vec<SkinInfo> = Vec::new();

        // Primary source: scan the runtime skin directory.
        if self.skin_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.skin_dir) {
                for entry in entries.flatten() {
                    let skin_json = entry.path().join("skin.json");
                    if skin_json.exists() {
                        if let Ok(content) = std::fs::read_to_string(&skin_json) {
                            if let Ok(skin) = serde_json::from_str::<SkinDefinition>(&content) {
                                let id = entry.file_name().to_string_lossy().to_string();
                                let is_builtin = self.builtin_skins.contains_key(&id);
                                skins.push(SkinInfo {
                                    id,
                                    name: skin.meta.name,
                                    version: skin.meta.version,
                                    author: skin.meta.author,
                                    description: skin.meta.description,
                                    is_builtin,
                                    has_images: !skin.backgrounds.is_empty() || !skin.images.is_empty(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Fallback: if nothing on disk yet (before seeding), show built-ins.
        if skins.is_empty() {
            for (id, def) in &self.builtin_skins {
                skins.push(SkinInfo {
                    id: id.clone(),
                    name: def.meta.name.clone(),
                    version: def.meta.version.clone(),
                    author: def.meta.author.clone(),
                    description: def.meta.description.clone(),
                    is_builtin: true,
                    has_images: !def.backgrounds.is_empty() || !def.images.is_empty(),
                });
            }
        }

        skins.sort_by(|a, b| a.id.cmp(&b.id));
        skins
    }

    /// Get current skin ID
    pub fn current_skin_id(&self) -> &str {
        &self.current_id
    }

    /// Get current skin definition
    pub fn current_skin(&self) -> &SkinDefinition {
        &self.current
    }

    /// Apply a skin by ID, returns CSS variables string
    pub fn apply_skin(&mut self, skin_id: &str) -> anyhow::Result<String> {
        // Try loading from disk first (user may have edited the skin.json)
        let skin_path = self.skin_dir.join(skin_id).join("skin.json");
        if skin_path.exists() {
            let content = std::fs::read_to_string(&skin_path)?;
            let skin: SkinDefinition = serde_json::from_str(&content)?;
            let css = self.generate_css(&skin, Some(skin_id));
            self.current = skin;
            self.current_id = skin_id.into();
            return Ok(css);
        }

        // Fall back to compiled-in built-in (before skins are seeded to disk)
        if let Some(skin) = self.builtin_skins.get(skin_id) {
            self.current = skin.clone();
            self.current_id = skin_id.into();
            Ok(self.css_variables())
        } else {
            anyhow::bail!("Skin not found: {}", skin_id)
        }
    }

    /// Generate CSS variables string from current skin
    pub fn css_variables(&self) -> String {
        self.generate_css(&self.current, None)
    }

    /// Read an image file and return a CSS data URI
    fn image_to_data_uri(abs_path: &Path) -> String {
        use base64::Engine;
        match std::fs::read(abs_path) {
            Ok(bytes) => {
                let mime = match abs_path.extension().and_then(|e| e.to_str()) {
                    Some("svg") => "image/svg+xml",
                    Some("png") => "image/png",
                    Some("jpg") | Some("jpeg") => "image/jpeg",
                    Some("gif") => "image/gif",
                    Some("webp") => "image/webp",
                    _ => "application/octet-stream",
                };
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                format!("data:{};base64,{}", mime, b64)
            }
            Err(e) => {
                eprintln!("[Skin] Failed to read image {:?}: {}", abs_path, e);
                String::new()
            }
        }
    }

    fn resolve_asset_path(&self, skin_id: Option<&str>, relative_path: &str) -> PathBuf {
        if let Some(id) = skin_id {
            self.skin_dir.join(id).join(relative_path)
        } else {
            // Built-in skin — no external assets
            PathBuf::from(relative_path)
        }
    }

    fn generate_css(&self, skin: &SkinDefinition, skin_id: Option<&str>) -> String {
        let c = &skin.colors;
        // Derive `--accent-rgb` (raw channels for rgba()) and `--accent-light`
        // (a pale tint) from the accent hex so every skin gets a coherent
        // palette without manually specifying dozens of tints.
        let rgb = parse_hex_color(&c.accent).unwrap_or((124, 108, 240));
        let trgb = parse_hex_color(&c.text_primary).unwrap_or((224, 212, 255));
        let accent_light = c
            .accent_light
            .clone()
            .unwrap_or_else(|| lighten_hex(rgb, 0.55));
        let mut css = format!(
            ":root {{\n\
             \x20 --bg-primary: {bg_p};\n\
             \x20 --bg-secondary: {bg_s};\n\
             \x20 --bg-tertiary: {bg_t};\n\
             \x20 --text-primary: {tx_p};\n\
             \x20 --text-primary-rgb: {tx_r}, {tx_g}, {tx_b};\n\
             \x20 --text-secondary: {tx_s};\n\
             \x20 --highlight: {hl};\n\
             \x20 --accent: {ac};\n\
             \x20 --accent-hover: {ac_h};\n\
             \x20 --accent-rgb: {ac_r}, {ac_g}, {ac_b};\n\
             \x20 --accent-light: {ac_l};\n\
             \x20 --progress-fill: {pf};\n\
             \x20 --progress-bg: {pb};\n\
             \x20 --border-color: {bc};\n\
             \x20 --panel-bg: {pan};\n\
             \x20 --button-bg: {bb};\n\
             \x20 --button-hover: {bh};\n\
             \x20 --border-radius: {br}px;\n\
             \x20 --font-title: {ft};\n\
             \x20 --font-body: {fb};\n",
            bg_p = c.bg_primary,
            bg_s = c.bg_secondary,
            bg_t = c.bg_tertiary,
            tx_p = c.text_primary,
            tx_r = trgb.0,
            tx_g = trgb.1,
            tx_b = trgb.2,
            tx_s = c.text_secondary,
            hl = c.highlight,
            ac = c.accent,
            ac_h = c.accent_hover,
            ac_r = rgb.0,
            ac_g = rgb.1,
            ac_b = rgb.2,
            ac_l = accent_light,
            pf = c.progress_fill,
            pb = c.progress_bg,
            bc = c.border_color,
            pan = c.panel_bg,
            bb = c.button_bg,
            bh = c.button_hover,
            br = skin.layout.border_radius,
            ft = skin.fonts.title,
            fb = skin.fonts.body,
        );

        if let Some(ref st) = c.spectrum_top {
            css.push_str(&format!("  --spectrum-top: {};\n", st));
        }
        if let Some(ref sb) = c.spectrum_bottom {
            css.push_str(&format!("  --spectrum-bottom: {};\n", sb));
        }
        if let Some(ref sp) = c.spectrum_peak {
            css.push_str(&format!("  --spectrum-peak: {};\n", sp));
        }
        if let Some(ref fm) = skin.fonts.mono {
            css.push_str(&format!("  --font-mono: {};\n", fm));
        }

        // Background images → CSS custom properties (embedded as data URIs)
        for (region, bg) in &skin.backgrounds {
            let abs_path = self.resolve_asset_path(skin_id, &bg.image);
            let data_uri = Self::image_to_data_uri(&abs_path);
            if !data_uri.is_empty() {
                css.push_str(&format!(
                    "  --bg-{}-image: url(\"{}\");\n",
                    region, data_uri
                ));
            }
            css.push_str(&format!("  --bg-{}-size: {};\n", region, bg.size));
            css.push_str(&format!("  --bg-{}-position: {};\n", region, bg.position));
            css.push_str(&format!("  --bg-{}-repeat: {};\n", region, bg.repeat));
        }

        // Button images → CSS custom properties (embedded as data URIs)
        for (name, imgs) in &skin.images {
            let abs_normal = self.resolve_asset_path(skin_id, &imgs.normal);
            let data_uri = Self::image_to_data_uri(&abs_normal);
            if !data_uri.is_empty() {
                css.push_str(&format!(
                    "  --img-{}-normal: url(\"{}\");\n",
                    name, data_uri
                ));
            }
            if let Some(ref hover) = imgs.hover {
                let abs = self.resolve_asset_path(skin_id, hover);
                let d = Self::image_to_data_uri(&abs);
                if !d.is_empty() {
                    css.push_str(&format!("  --img-{}-hover: url(\"{}\");\n", name, d));
                }
            }
            if let Some(ref pressed) = imgs.pressed {
                let abs = self.resolve_asset_path(skin_id, pressed);
                let d = Self::image_to_data_uri(&abs);
                if !d.is_empty() {
                    css.push_str(&format!("  --img-{}-pressed: url(\"{}\");\n", name, d));
                }
            }
            if let Some(ref disabled) = imgs.disabled {
                let abs = self.resolve_asset_path(skin_id, disabled);
                let d = Self::image_to_data_uri(&abs);
                if !d.is_empty() {
                    css.push_str(&format!("  --img-{}-disabled: url(\"{}\");\n", name, d));
                }
            }
        }

        css.push_str("}\n");

        // ── Light-theme overrides ──────────────────────────────────────
        // When <html data-theme="light"> the player inverts its colour
        // scheme: backgrounds become light, text becomes dark, while the
        // skin's accent / spectrum / font choices are preserved so the
        // user's personality still shows through.
        css.push_str(&format!(
            ":root[data-theme=\"light\"] {{\n\
             \x20 --bg-primary: #f2f2f2;\n\
             \x20 --bg-secondary: #e8e8e8;\n\
             \x20 --bg-tertiary: #dcdcdc;\n\
             \x20 --text-primary: #1a1a1a;\n\
             \x20 --text-primary-rgb: 26, 26, 26;\n\
             \x20 --text-secondary: #666666;\n\
             \x20 --highlight: #333333;\n\
             \x20 --accent: {ac};\n\
             \x20 --accent-hover: {ac_h};\n\
             \x20 --accent-rgb: {ac_r}, {ac_g}, {ac_b};\n\
             \x20 --accent-light: {ac_l};\n\
             \x20 --progress-fill: {pf};\n\
             \x20 --progress-bg: #d4d4d4;\n\
             \x20 --border-color: #d0d0d0;\n\
             \x20 --panel-bg: #f2f2f2;\n\
             \x20 --button-bg: #e6e6e6;\n\
             \x20 --button-hover: #d8d8d8;\n",
            ac = c.accent,
            ac_h = c.accent_hover,
            ac_r = rgb.0,
            ac_g = rgb.1,
            ac_b = rgb.2,
            ac_l = accent_light,
            pf = c.progress_fill,
        ));
        if let Some(ref st) = c.spectrum_top {
            css.push_str(&format!("  --spectrum-top: {};\n", st));
        }
        if let Some(ref sb) = c.spectrum_bottom {
            css.push_str(&format!("  --spectrum-bottom: {};\n", sb));
        }
        if let Some(ref sp) = c.spectrum_peak {
            css.push_str(&format!("  --spectrum-peak: {};\n", sp));
        }
        css.push_str("}\n");

        // Background images → CSS custom properties only (consumed by MainPanel.module.css)
        // No separate selector rules needed — the module CSS uses background shorthand
        // with var() fallbacks to layer images on top of color gradients.

        // Button image rules — use [data-skin-btn] attribute to bypass CSS Modules hashing
        for (name, imgs) in &skin.images {
            let selector = format!("[data-skin-btn=\"{}\"]", name);
            css.push_str(&format!(
                "{sel} {{\n\
                 \x20 background-image: var(--img-{name}-normal) !important;\n\
                 \x20 background-size: contain !important;\n\
                 \x20 background-repeat: no-repeat !important;\n\
                 \x20 background-position: center !important;\n\
                 \x20 background-color: transparent !important;\n\
                 \x20 color: transparent !important;\n\
                 \x20 font-size: 0 !important;\n\
                 }}\n",
                sel = selector,
                name = name,
            ));
            if imgs.hover.is_some() {
                css.push_str(&format!(
                    "{sel}:hover {{ background-image: var(--img-{name}-hover) !important; }}\n",
                    sel = selector,
                    name = name,
                ));
            }
            if imgs.pressed.is_some() {
                css.push_str(&format!(
                    "{sel}:active {{ background-image: var(--img-{name}-pressed) !important; }}\n",
                    sel = selector,
                    name = name,
                ));
            }
        }

        if let Some(ref custom) = skin.custom_css {
            css.push_str("\n");
            css.push_str(custom);
        }

        css
    }

    /// Install a .ttskin package (zip file)
    pub fn install_skin(&mut self, path: &Path) -> anyhow::Result<SkinInfo> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        // Find skin.json in archive
        let skin_json = {
            let mut found = None;
            for i in 0..archive.len() {
                let entry = archive.by_index(i)?;
                if entry.name().ends_with("skin.json") {
                    found = Some(i);
                    break;
                }
            }
            found.ok_or_else(|| anyhow::anyhow!("No skin.json found in archive"))?
        };

        let mut entry = archive.by_index(skin_json)?;
        let mut content = String::new();
        use std::io::Read;
        entry.read_to_string(&mut content)?;
        drop(entry);

        let skin: SkinDefinition = serde_json::from_str(&content)?;
        let skin_id = skin.meta.name.to_lowercase().replace(' ', "-");
        let install_dir = self.skin_dir.join(&skin_id);
        std::fs::create_dir_all(&install_dir)?;

        // Extract all files.
        //
        // Zip-slip protection: `enclosed_name()` (provided by the `zip` crate)
        // returns `None` for entries whose path is absolute or contains `..`
        // components, preventing a malicious `.ttskin` from writing outside
        // `install_dir` (e.g. `../../evil.exe`). Unsafe entries are skipped.
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let outpath = match entry.enclosed_name() {
                Some(p) => install_dir.join(p),
                None => {
                    tracing::warn!("Skipping unsafe entry in skin archive: {:?}", entry.name());
                    continue;
                }
            };
            if entry.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut entry, &mut outfile)?;
            }
        }

        Ok(SkinInfo {
            id: skin_id,
            name: skin.meta.name.clone(),
            version: skin.meta.version.clone(),
            author: skin.meta.author.clone(),
            description: skin.meta.description.clone(),
            is_builtin: false,
            has_images: !skin.backgrounds.is_empty() || !skin.images.is_empty(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only constructor with a custom skin directory.
    fn make_mgr_with_dir(dir: PathBuf) -> SkinManager {
        let builtin = SkinManager::builtin_skins();
        SkinManager {
            current: builtin["default"].clone(),
            current_id: "default".into(),
            builtin_skins: builtin,
            skin_dir: dir,
        }
    }

    /// Create a unique temp directory for testing.
    fn temp_skin_dir() -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ttplayer-skin-test-{}", ts))
    }

    #[test]
    fn builtin_skins_loads_all_four() {
        let skins = SkinManager::builtin_skins();
        assert_eq!(skins.len(), 4, "expected 4 builtin skins");
        assert!(skins.contains_key("default"));
        assert!(skins.contains_key("ttplayer-blue"));
        assert!(skins.contains_key("green"));
        assert!(skins.contains_key("rose"));
    }

    #[test]
    fn default_skin_matches_legacy_values() {
        let skins = SkinManager::builtin_skins();
        let def = &skins["default"];
        assert_eq!(def.meta.name, "紫韵经典");
        assert_eq!(def.meta.author, "TTPlayer");
        assert_eq!(def.colors.accent, "#7c3aed");
        assert_eq!(def.colors.bg_primary, "#0f061a");
        assert_eq!(def.layout.border_radius, 4);
        assert_eq!(def.fonts.title, "'Segoe UI', 'Microsoft YaHei', sans-serif");
        assert!(def.fonts.mono.is_none());
        assert!(def.colors.spectrum_top.is_none());
    }

    #[test]
    fn ttplayer_blue_preserves_spectrum_and_mono_font() {
        let skins = SkinManager::builtin_skins();
        let t = &skins["ttplayer-blue"];
        assert_eq!(t.colors.accent, "#4a8db7");
        assert_eq!(t.colors.spectrum_top.as_deref(), Some("#27435f"));
        assert_eq!(t.colors.spectrum_peak.as_deref(), Some("#88aacb"));
        assert_eq!(t.layout.border_radius, 3);
        assert_eq!(t.fonts.mono.as_deref(), Some("'Courier New', monospace"));
    }

    #[test]
    fn new_uses_default_as_current() {
        let mgr = SkinManager::new();
        assert_eq!(mgr.current_skin_id(), "default");
        assert_eq!(mgr.current_skin().meta.name, "紫韵经典");
    }

    #[test]
    fn list_skins_includes_all_builtins() {
        // Use a temp dir (doesn't exist) so list_skins falls back to
        // compiled-in built-ins — the behavior before disk seeding.
        let dir = temp_skin_dir();
        let mgr = make_mgr_with_dir(dir.clone());
        let list = mgr.list_skins();
        for id in &["default", "ttplayer-blue", "green", "rose"] {
            let found = list.iter().find(|s| s.id == *id);
            assert!(found.is_some(), "missing builtin skin: {}", id);
            assert!(found.unwrap().is_builtin, "{} should be builtin", id);
        }
    }

    #[test]
    fn css_emits_accent_rgb_and_light() {
        let mut mgr = SkinManager::new();
        // default skin — accent_light is explicit in JSON
        let css = mgr.css_variables();
        assert!(
            css.contains("--accent-rgb:"),
            "default CSS must contain --accent-rgb"
        );
        assert!(
            css.contains("--accent-light: #C4B5FD;"),
            "default CSS must contain explicit --accent-light"
        );

        // ttplayer-blue — accent #4a8db7 → rgb 74,141,183
        let css = mgr.apply_skin("ttplayer-blue").unwrap();
        assert!(css.contains("--accent-rgb: 74, 141, 183;"));
        assert!(css.contains("--accent-light: #90e0ef;"));

        // green — accent #22c55e → rgb 34,197,94
        let css = mgr.apply_skin("green").unwrap();
        assert!(css.contains("--accent-rgb: 34, 197, 94;"));
        assert!(css.contains("--accent-light: #86efac;"));

        // rose — accent #e11d48 → rgb 225,29,72
        let css = mgr.apply_skin("rose").unwrap();
        assert!(css.contains("--accent-rgb: 225, 29, 72;"));
        assert!(css.contains("--accent-light: #fda4af;"));
    }

    #[test]
    fn parse_hex_color_handles_short_and_long() {
        assert_eq!(parse_hex_color("#7c3aed"), Some((124, 58, 237)));
        assert_eq!(parse_hex_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_color("000"), Some((0, 0, 0)));
        assert_eq!(parse_hex_color("#zzz"), None);
    }

    #[test]
    fn css_includes_light_theme_block() {
        let mut mgr = SkinManager::new();
        let css = mgr.css_variables();
        assert!(
            css.contains("data-theme=\"light\""),
            "CSS must contain a light-theme override block"
        );
        assert!(
            css.contains("--bg-primary: #f2f2f2;"),
            "light block must override --bg-primary"
        );
        assert!(
            css.contains("--text-primary: #1a1a1a;"),
            "light block must override --text-primary"
        );
        // Accent should be preserved from the skin
        assert!(css.contains("--accent: #7c3aed;"));
    }

    #[test]
    fn lighten_hex_mixes_toward_white() {
        // 50% mix of black → mid gray
        assert_eq!(lighten_hex((0, 0, 0), 0.5), "#808080");
        // 0% leaves colour unchanged
        assert_eq!(lighten_hex((124, 58, 237), 0.0), "#7c3aed");
    }

    #[test]
    fn ensure_skins_seeds_all_on_first_run() {
        let dir = temp_skin_dir();
        let mgr = make_mgr_with_dir(dir.clone());

        // Directory doesn't exist yet → all 4 should be seeded.
        mgr.ensure_skins_on_disk().unwrap();

        for id in &["default", "ttplayer-blue", "green", "rose"] {
            assert!(
                dir.join(id).join("skin.json").exists(),
                "skin '{}' should be seeded",
                id
            );
        }

        let list = mgr.list_skins();
        assert_eq!(list.len(), 4);
        for s in &list {
            assert!(s.is_builtin, "{} should be is_builtin", s.id);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_skins_restores_missing_builtins() {
        let dir = temp_skin_dir();
        let mgr = make_mgr_with_dir(dir.clone());
        mgr.ensure_skins_on_disk().unwrap();

        // Delete two built-in skins from disk.
        std::fs::remove_dir_all(dir.join("rose")).unwrap();
        std::fs::remove_dir_all(dir.join("ttplayer-blue")).unwrap();
        assert!(!dir.join("rose").exists());
        assert!(!dir.join("ttplayer-blue").exists());

        // Next run: they should be re-seeded (never overwrite existing).
        let mgr2 = make_mgr_with_dir(dir.clone());
        mgr2.ensure_skins_on_disk().unwrap();

        assert!(dir.join("rose").join("skin.json").exists(), "rose should be restored");
        assert!(dir.join("ttplayer-blue").join("skin.json").exists(), "ttplayer-blue should be restored");
        // Default was never deleted → should still be there.
        assert!(dir.join("default").join("skin.json").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_skin_rejects_default() {
        let dir = temp_skin_dir();
        let mgr = make_mgr_with_dir(dir.clone());
        mgr.ensure_skins_on_disk().unwrap();

        let result = mgr.delete_skin("default");
        assert!(result.is_err(), "deleting default should fail");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_skin_removes_from_disk() {
        let dir = temp_skin_dir();
        let mgr = make_mgr_with_dir(dir.clone());
        mgr.ensure_skins_on_disk().unwrap();
        assert!(dir.join("ttplayer-blue").exists());

        mgr.delete_skin("ttplayer-blue").unwrap();
        assert!(!dir.join("ttplayer-blue").exists(), "ttplayer-blue should be deleted");

        // list_skins should no longer include ttplayer-blue.
        let list = mgr.list_skins();
        assert!(!list.iter().any(|s| s.id == "ttplayer-blue"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_skin_reads_edited_name_from_disk() {
        let dir = temp_skin_dir();
        let mgr = make_mgr_with_dir(dir.clone());
        mgr.ensure_skins_on_disk().unwrap();

        // User edits the skin.json on disk — change the name.
        let path = dir.join("ttplayer-blue").join("skin.json");
        let content = std::fs::read_to_string(&path).unwrap();
        let mut skin: serde_json::Value = serde_json::from_str(&content).unwrap();
        skin["name"] = serde_json::json!("我的自定义蓝");
        std::fs::write(&path, serde_json::to_string_pretty(&skin).unwrap()).unwrap();

        // list_skins should reflect the edited name.
        let list = mgr.list_skins();
        let s = list.iter().find(|s| s.id == "ttplayer-blue").unwrap();
        assert_eq!(s.name, "我的自定义蓝");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
