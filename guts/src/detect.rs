use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Url,
    Email,
    Number,
    Ip,
    Text,
}

static URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(https?://|www\.).+").expect("valid url regex"));
static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").expect("valid email regex"));
static NUMBER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^-?\d+(\.\d+)?$").expect("valid number regex"));
static IP_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$")
        .expect("valid ip regex")
});

pub fn detect_kind(value: &str) -> CellKind {
    let value = value.trim();
    if value.is_empty() {
        return CellKind::Text;
    }
    if URL_RE.is_match(value) {
        CellKind::Url
    } else if EMAIL_RE.is_match(value) {
        CellKind::Email
    } else if IP_RE.is_match(value) {
        CellKind::Ip
    } else if NUMBER_RE.is_match(value) {
        CellKind::Number
    } else {
        CellKind::Text
    }
}
