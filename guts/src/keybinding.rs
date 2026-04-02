use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeybindingConfig {
    pub navigation: NavigationKeybindingConfig,
    pub search: SearchKeybindingConfig,
    pub actions: ActionKeybindingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NavigationKeybindingConfig {
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub up: Vec<String>,
    pub down: Vec<String>,
    pub top: Vec<String>,
    pub bottom: Vec<String>,
    pub page_up: Vec<String>,
    pub page_down: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchKeybindingConfig {
    pub search_mode: Vec<String>,
    pub query_mode: Vec<String>,
    pub fuzzy_mode: Vec<String>,
    pub fuzzy_cycle_scope: Vec<String>,
    pub next_match: Vec<String>,
    pub prev_match: Vec<String>,
    pub history_prev: Vec<String>,
    pub history_next: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ActionKeybindingConfig {
    pub open: Vec<String>,
    pub copy: Vec<String>,
    pub quit: Vec<String>,
    pub confirm: Vec<String>,
    pub cancel: Vec<String>,
    pub backspace: Vec<String>,
}

impl Default for NavigationKeybindingConfig {
    fn default() -> Self {
        Self {
            left: vec!["h".to_string(), "Left".to_string()],
            right: vec!["l".to_string(), "Right".to_string()],
            up: vec!["k".to_string(), "Up".to_string()],
            down: vec!["j".to_string(), "Down".to_string()],
            top: vec!["g".to_string()],
            bottom: vec!["G".to_string()],
            page_up: vec!["PageUp".to_string()],
            page_down: vec!["PageDown".to_string()],
        }
    }
}

impl Default for SearchKeybindingConfig {
    fn default() -> Self {
        Self {
            search_mode: vec!["/".to_string()],
            query_mode: vec![":".to_string()],
            fuzzy_mode: vec!["Ctrl-f".to_string()],
            fuzzy_cycle_scope: vec!["Tab".to_string()],
            next_match: vec!["n".to_string()],
            prev_match: vec!["N".to_string()],
            history_prev: vec!["Ctrl-p".to_string()],
            history_next: vec!["Ctrl-n".to_string()],
        }
    }
}

impl Default for ActionKeybindingConfig {
    fn default() -> Self {
        Self {
            open: vec!["o".to_string()],
            copy: vec!["y".to_string()],
            quit: vec!["q".to_string()],
            confirm: vec!["Enter".to_string()],
            cancel: vec!["Esc".to_string()],
            backspace: vec!["Backspace".to_string()],
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyBinding {
    code: KeyCode,
    modifiers: KeyModifiers,
    label: String,
}

impl KeyBinding {
    fn matches(&self, event: KeyEvent) -> bool {
        if event.code != self.code {
            return false;
        }
        let important = event.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        important == self.modifiers
    }
}

#[derive(Debug, Clone)]
pub struct Keymap {
    pub left: Vec<KeyBinding>,
    pub right: Vec<KeyBinding>,
    pub up: Vec<KeyBinding>,
    pub down: Vec<KeyBinding>,
    pub top: Vec<KeyBinding>,
    pub bottom: Vec<KeyBinding>,
    pub page_up: Vec<KeyBinding>,
    pub page_down: Vec<KeyBinding>,
    pub search_mode: Vec<KeyBinding>,
    pub query_mode: Vec<KeyBinding>,
    pub fuzzy_mode: Vec<KeyBinding>,
    pub fuzzy_cycle_scope: Vec<KeyBinding>,
    pub next_match: Vec<KeyBinding>,
    pub prev_match: Vec<KeyBinding>,
    pub history_prev: Vec<KeyBinding>,
    pub history_next: Vec<KeyBinding>,
    pub open: Vec<KeyBinding>,
    pub copy: Vec<KeyBinding>,
    pub quit: Vec<KeyBinding>,
    pub confirm: Vec<KeyBinding>,
    pub cancel: Vec<KeyBinding>,
    pub backspace: Vec<KeyBinding>,
}

impl Keymap {
    pub fn from_config(config: &KeybindingConfig) -> Self {
        Self {
            left: parse_binding_group(&config.navigation.left, &["h", "Left"]),
            right: parse_binding_group(&config.navigation.right, &["l", "Right"]),
            up: parse_binding_group(&config.navigation.up, &["k", "Up"]),
            down: parse_binding_group(&config.navigation.down, &["j", "Down"]),
            top: parse_binding_group(&config.navigation.top, &["g"]),
            bottom: parse_binding_group(&config.navigation.bottom, &["G"]),
            page_up: parse_binding_group(&config.navigation.page_up, &["PageUp"]),
            page_down: parse_binding_group(&config.navigation.page_down, &["PageDown"]),
            search_mode: parse_binding_group(&config.search.search_mode, &["/"]),
            query_mode: parse_binding_group(&config.search.query_mode, &[":"]),
            fuzzy_mode: parse_binding_group(&config.search.fuzzy_mode, &["Ctrl-f"]),
            fuzzy_cycle_scope: parse_binding_group(&config.search.fuzzy_cycle_scope, &["Tab"]),
            next_match: parse_binding_group(&config.search.next_match, &["n"]),
            prev_match: parse_binding_group(&config.search.prev_match, &["N"]),
            history_prev: parse_binding_group(&config.search.history_prev, &["Ctrl-p"]),
            history_next: parse_binding_group(&config.search.history_next, &["Ctrl-n"]),
            open: parse_binding_group(&config.actions.open, &["o"]),
            copy: parse_binding_group(&config.actions.copy, &["y"]),
            quit: parse_binding_group(&config.actions.quit, &["q"]),
            confirm: parse_binding_group(&config.actions.confirm, &["Enter"]),
            cancel: parse_binding_group(&config.actions.cancel, &["Esc"]),
            backspace: parse_binding_group(&config.actions.backspace, &["Backspace"]),
        }
    }

    pub fn is_match(bindings: &[KeyBinding], event: KeyEvent) -> bool {
        bindings.iter().any(|binding| binding.matches(event))
    }

    pub fn labels(bindings: &[KeyBinding]) -> String {
        bindings
            .iter()
            .map(|binding| binding.label.clone())
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn parse_binding_group(raw: &[String], fallback: &[&str]) -> Vec<KeyBinding> {
    let mut parsed = raw
        .iter()
        .filter_map(|value| parse_key_binding(value))
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        parsed = fallback
            .iter()
            .filter_map(|value| parse_key_binding(value))
            .collect::<Vec<_>>();
    }
    parsed
}

fn parse_key_binding(raw: &str) -> Option<KeyBinding> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return None;
    }

    let parts = normalized.split('-').collect::<Vec<_>>();
    let mut modifiers = KeyModifiers::empty();
    let key_token = if parts.len() > 1 {
        for part in &parts[..parts.len() - 1] {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "option" => modifiers |= KeyModifiers::ALT,
                "shift" => {}
                _ => {}
            }
        }
        parts[parts.len() - 1]
    } else {
        normalized
    };

    let code = parse_key_code(key_token)?;
    Some(KeyBinding {
        code,
        modifiers,
        label: normalized.to_string(),
    })
}

fn parse_key_code(token: &str) -> Option<KeyCode> {
    let normalized = token.trim();
    if normalized.len() == 1 {
        return normalized.chars().next().map(KeyCode::Char);
    }

    match normalized.to_ascii_lowercase().as_str() {
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "pageup" | "pgup" => Some(KeyCode::PageUp),
        "pagedown" | "pgdown" | "pgdn" => Some(KeyCode::PageDown),
        "enter" | "return" => Some(KeyCode::Enter),
        "esc" | "escape" => Some(KeyCode::Esc),
        "backspace" => Some(KeyCode::Backspace),
        "tab" => Some(KeyCode::Tab),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "space" => Some(KeyCode::Char(' ')),
        _ => None,
    }
}
