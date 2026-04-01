use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use ratatui::style::Color;
use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub border: Color,
    pub header_background: Color,
    pub header_foreground: Color,
    pub row_foreground: Color,
    pub selected_background: Color,
    pub selected_foreground: Color,
    pub match_background: Color,
    pub status_mode_foreground: Color,
    pub status_mode_background: Color,
    pub status_text_foreground: Color,
    pub metrics_foreground: Color,
    pub column_foreground: Color,
    pub type_foreground: Color,
    pub source_foreground: Color,
    pub help_foreground: Color,
    pub input_prompt_foreground: Color,
    pub input_text_foreground: Color,
}

#[derive(Debug, Clone)]
pub struct ActiveTheme {
    pub palette: Palette,
    name: String,
    fallback_reason: Option<String>,
}

pub enum InitConfigOutcome {
    Created(PathBuf),
    AlreadyExists(PathBuf),
}

pub const DEFAULT_THEME_TOML: &str = r##"preset = "nord"

[colors]
# Any field in this section is optional.
# Use #RRGGBB for TrueColor values, or ANSI names like "blue", "light_cyan", "gray".
# border = "#81A1C1"
# selected_background = "#5E81AC"
# status_mode_background = "#A3BE8C"
"##;

impl ActiveTheme {
    pub fn initial_status(&self) -> String {
        match &self.fallback_reason {
            Some(reason) => format!("Ready | Theme: {} ({reason})", self.name),
            None => format!("Ready | Theme: {}", self.name),
        }
    }

    fn configured(name: String, palette: Palette) -> Self {
        Self {
            palette,
            name,
            fallback_reason: None,
        }
    }

    fn ansi_fallback(reason: impl Into<String>) -> Self {
        Self {
            palette: ansi_palette(),
            name: "ansi-basic".to_string(),
            fallback_reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ThemeConfig {
    preset: Option<String>,
    colors: ThemeOverrides,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ThemeOverrides {
    background: Option<String>,
    border: Option<String>,
    header_background: Option<String>,
    header_foreground: Option<String>,
    row_foreground: Option<String>,
    selected_background: Option<String>,
    selected_foreground: Option<String>,
    match_background: Option<String>,
    status_mode_foreground: Option<String>,
    status_mode_background: Option<String>,
    status_text_foreground: Option<String>,
    metrics_foreground: Option<String>,
    column_foreground: Option<String>,
    type_foreground: Option<String>,
    source_foreground: Option<String>,
    help_foreground: Option<String>,
    input_prompt_foreground: Option<String>,
    input_text_foreground: Option<String>,
}

pub fn load_active_theme() -> ActiveTheme {
    let Some(path) = discover_theme_file() else {
        return ActiveTheme::ansi_fallback("missing theme.toml");
    };

    if !supports_truecolor() {
        return ActiveTheme::ansi_fallback("terminal has no TrueColor");
    }

    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(_) => return ActiveTheme::ansi_fallback("cannot read theme.toml"),
    };

    let config: ThemeConfig = match toml::from_str(&raw) {
        Ok(config) => config,
        Err(_) => return ActiveTheme::ansi_fallback("invalid theme.toml"),
    };

    let (base_name, mut palette) = base_palette_for(config.preset.as_deref());
    if apply_overrides(&mut palette, &config.colors).is_err() {
        return ActiveTheme::ansi_fallback("invalid color in theme.toml");
    }

    let name = if has_any_override(&config.colors) {
        format!("custom ({base_name})")
    } else {
        base_name.to_string()
    };

    ActiveTheme::configured(name, palette)
}

pub fn init_default_config() -> io::Result<InitConfigOutcome> {
    let path = default_user_theme_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if path.exists() {
        return Ok(InitConfigOutcome::AlreadyExists(path));
    }

    fs::write(&path, DEFAULT_THEME_TOML)?;
    Ok(InitConfigOutcome::Created(path))
}

fn default_user_theme_path() -> io::Result<PathBuf> {
    let home =
        env::var("HOME").map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join(".config/guts/theme.toml"))
}

fn discover_theme_file() -> Option<PathBuf> {
    if let Ok(raw) = env::var("GUTS_THEME_FILE") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Some(path);
        }
    }

    let local = PathBuf::from("theme.toml");
    if local.is_file() {
        return Some(local);
    }

    if let Ok(xdg_home) = env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg_home).join("guts/theme.toml");
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join(".config/guts/theme.toml");
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

fn supports_truecolor() -> bool {
    let colorterm = env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return true;
    }

    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    term.contains("truecolor") || term.contains("direct")
}

fn has_any_override(colors: &ThemeOverrides) -> bool {
    [
        &colors.background,
        &colors.border,
        &colors.header_background,
        &colors.header_foreground,
        &colors.row_foreground,
        &colors.selected_background,
        &colors.selected_foreground,
        &colors.match_background,
        &colors.status_mode_foreground,
        &colors.status_mode_background,
        &colors.status_text_foreground,
        &colors.metrics_foreground,
        &colors.column_foreground,
        &colors.type_foreground,
        &colors.source_foreground,
        &colors.help_foreground,
        &colors.input_prompt_foreground,
        &colors.input_text_foreground,
    ]
    .iter()
    .any(|value| value.is_some())
}

fn base_palette_for(preset: Option<&str>) -> (&'static str, Palette) {
    match preset.map(|p| p.trim().to_ascii_lowercase()).as_deref() {
        Some("nord") => ("nord", nord_palette()),
        Some("gravbox") | Some("gruvbox") => ("gravbox", gravbox_palette()),
        Some("catppuccin") => ("catppuccin", catppuccin_palette()),
        Some("monochrome") => ("monochrome", monochrome_palette()),
        Some("ansi") | Some("basic") => ("ansi-basic", ansi_palette()),
        Some(_) => ("nord", nord_palette()),
        None => ("nord", nord_palette()),
    }
}

fn apply_overrides(palette: &mut Palette, overrides: &ThemeOverrides) -> Result<(), String> {
    apply_optional_color(&mut palette.background, &overrides.background, "background")?;
    apply_optional_color(&mut palette.border, &overrides.border, "border")?;
    apply_optional_color(
        &mut palette.header_background,
        &overrides.header_background,
        "header_background",
    )?;
    apply_optional_color(
        &mut palette.header_foreground,
        &overrides.header_foreground,
        "header_foreground",
    )?;
    apply_optional_color(
        &mut palette.row_foreground,
        &overrides.row_foreground,
        "row_foreground",
    )?;
    apply_optional_color(
        &mut palette.selected_background,
        &overrides.selected_background,
        "selected_background",
    )?;
    apply_optional_color(
        &mut palette.selected_foreground,
        &overrides.selected_foreground,
        "selected_foreground",
    )?;
    apply_optional_color(
        &mut palette.match_background,
        &overrides.match_background,
        "match_background",
    )?;
    apply_optional_color(
        &mut palette.status_mode_foreground,
        &overrides.status_mode_foreground,
        "status_mode_foreground",
    )?;
    apply_optional_color(
        &mut palette.status_mode_background,
        &overrides.status_mode_background,
        "status_mode_background",
    )?;
    apply_optional_color(
        &mut palette.status_text_foreground,
        &overrides.status_text_foreground,
        "status_text_foreground",
    )?;
    apply_optional_color(
        &mut palette.metrics_foreground,
        &overrides.metrics_foreground,
        "metrics_foreground",
    )?;
    apply_optional_color(
        &mut palette.column_foreground,
        &overrides.column_foreground,
        "column_foreground",
    )?;
    apply_optional_color(
        &mut palette.type_foreground,
        &overrides.type_foreground,
        "type_foreground",
    )?;
    apply_optional_color(
        &mut palette.source_foreground,
        &overrides.source_foreground,
        "source_foreground",
    )?;
    apply_optional_color(
        &mut palette.help_foreground,
        &overrides.help_foreground,
        "help_foreground",
    )?;
    apply_optional_color(
        &mut palette.input_prompt_foreground,
        &overrides.input_prompt_foreground,
        "input_prompt_foreground",
    )?;
    apply_optional_color(
        &mut palette.input_text_foreground,
        &overrides.input_text_foreground,
        "input_text_foreground",
    )?;
    Ok(())
}

fn apply_optional_color(
    slot: &mut Color,
    value: &Option<String>,
    field: &str,
) -> Result<(), String> {
    let Some(raw) = value else {
        return Ok(());
    };
    *slot = parse_color(raw).map_err(|err| format!("{field}: {err}"))?;
    Ok(())
}

fn parse_color(raw: &str) -> Result<Color, String> {
    let value = raw.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return Err("hex color must be #RRGGBB".to_string());
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "invalid hex color".to_string())?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "invalid hex color".to_string())?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "invalid hex color".to_string())?;
        return Ok(Color::Rgb(r, g, b));
    }

    match value.to_ascii_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "white" => Ok(Color::White),
        "gray" | "grey" => Ok(Color::Gray),
        "dark_gray" | "dark_grey" => Ok(Color::DarkGray),
        "light_red" => Ok(Color::LightRed),
        "light_green" => Ok(Color::LightGreen),
        "light_yellow" => Ok(Color::LightYellow),
        "light_blue" => Ok(Color::LightBlue),
        "light_magenta" => Ok(Color::LightMagenta),
        "light_cyan" => Ok(Color::LightCyan),
        _ => Err("unsupported color value".to_string()),
    }
}

fn ansi_palette() -> Palette {
    Palette {
        background: Color::Black,
        border: Color::Blue,
        header_background: Color::DarkGray,
        header_foreground: Color::White,
        row_foreground: Color::Gray,
        selected_background: Color::Blue,
        selected_foreground: Color::White,
        match_background: Color::DarkGray,
        status_mode_foreground: Color::Black,
        status_mode_background: Color::Green,
        status_text_foreground: Color::White,
        metrics_foreground: Color::Cyan,
        column_foreground: Color::LightBlue,
        type_foreground: Color::Yellow,
        source_foreground: Color::Gray,
        help_foreground: Color::Gray,
        input_prompt_foreground: Color::LightCyan,
        input_text_foreground: Color::White,
    }
}

fn nord_palette() -> Palette {
    Palette {
        background: Color::Rgb(46, 52, 64),
        border: Color::Rgb(94, 129, 172),
        header_background: Color::Rgb(59, 66, 82),
        header_foreground: Color::Rgb(236, 239, 244),
        row_foreground: Color::Rgb(216, 222, 233),
        selected_background: Color::Rgb(94, 129, 172),
        selected_foreground: Color::Rgb(236, 239, 244),
        match_background: Color::Rgb(76, 86, 106),
        status_mode_foreground: Color::Rgb(46, 52, 64),
        status_mode_background: Color::Rgb(163, 190, 140),
        status_text_foreground: Color::Rgb(236, 239, 244),
        metrics_foreground: Color::Rgb(136, 192, 208),
        column_foreground: Color::Rgb(129, 161, 193),
        type_foreground: Color::Rgb(235, 203, 139),
        source_foreground: Color::Rgb(176, 184, 198),
        help_foreground: Color::Rgb(176, 184, 198),
        input_prompt_foreground: Color::Rgb(136, 192, 208),
        input_text_foreground: Color::Rgb(236, 239, 244),
    }
}

fn gravbox_palette() -> Palette {
    Palette {
        background: Color::Rgb(40, 40, 40),
        border: Color::Rgb(215, 153, 33),
        header_background: Color::Rgb(60, 56, 54),
        header_foreground: Color::Rgb(251, 241, 199),
        row_foreground: Color::Rgb(213, 196, 161),
        selected_background: Color::Rgb(181, 118, 20),
        selected_foreground: Color::Rgb(251, 241, 199),
        match_background: Color::Rgb(102, 92, 84),
        status_mode_foreground: Color::Rgb(40, 40, 40),
        status_mode_background: Color::Rgb(184, 187, 38),
        status_text_foreground: Color::Rgb(251, 241, 199),
        metrics_foreground: Color::Rgb(131, 165, 152),
        column_foreground: Color::Rgb(250, 189, 47),
        type_foreground: Color::Rgb(254, 128, 25),
        source_foreground: Color::Rgb(168, 153, 132),
        help_foreground: Color::Rgb(168, 153, 132),
        input_prompt_foreground: Color::Rgb(142, 192, 124),
        input_text_foreground: Color::Rgb(251, 241, 199),
    }
}

fn catppuccin_palette() -> Palette {
    Palette {
        background: Color::Rgb(30, 30, 46),
        border: Color::Rgb(137, 180, 250),
        header_background: Color::Rgb(49, 50, 68),
        header_foreground: Color::Rgb(205, 214, 244),
        row_foreground: Color::Rgb(186, 194, 222),
        selected_background: Color::Rgb(88, 91, 112),
        selected_foreground: Color::Rgb(238, 241, 255),
        match_background: Color::Rgb(108, 112, 134),
        status_mode_foreground: Color::Rgb(30, 30, 46),
        status_mode_background: Color::Rgb(166, 227, 161),
        status_text_foreground: Color::Rgb(205, 214, 244),
        metrics_foreground: Color::Rgb(137, 220, 235),
        column_foreground: Color::Rgb(137, 180, 250),
        type_foreground: Color::Rgb(249, 226, 175),
        source_foreground: Color::Rgb(166, 173, 200),
        help_foreground: Color::Rgb(166, 173, 200),
        input_prompt_foreground: Color::Rgb(137, 180, 250),
        input_text_foreground: Color::Rgb(205, 214, 244),
    }
}

fn monochrome_palette() -> Palette {
    Palette {
        background: Color::Rgb(17, 17, 17),
        border: Color::Rgb(119, 119, 119),
        header_background: Color::Rgb(27, 27, 27),
        header_foreground: Color::Rgb(224, 224, 224),
        row_foreground: Color::Rgb(192, 192, 192),
        selected_background: Color::Rgb(58, 58, 58),
        selected_foreground: Color::Rgb(255, 255, 255),
        match_background: Color::Rgb(43, 43, 43),
        status_mode_foreground: Color::Rgb(17, 17, 17),
        status_mode_background: Color::Rgb(176, 176, 176),
        status_text_foreground: Color::Rgb(224, 224, 224),
        metrics_foreground: Color::Rgb(208, 208, 208),
        column_foreground: Color::Rgb(200, 200, 200),
        type_foreground: Color::Rgb(184, 184, 184),
        source_foreground: Color::Rgb(150, 150, 150),
        help_foreground: Color::Rgb(150, 150, 150),
        input_prompt_foreground: Color::Rgb(224, 224, 224),
        input_text_foreground: Color::Rgb(255, 255, 255),
    }
}
