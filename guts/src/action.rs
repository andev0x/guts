use arboard::Clipboard;

use crate::detect::{detect_kind, CellKind};
use crate::error::{AppError, AppResult};

pub struct Action;

impl Action {
    pub fn open(value: &str) -> AppResult<()> {
        let kind = detect_kind(value);
        let target = match kind {
            CellKind::Url => normalize_url(value),
            CellKind::Email => format!("mailto:{}", value.trim()),
            _ => {
                return Err(AppError::Action(
                    "Selected cell is not an openable link or email".to_string(),
                ));
            }
        };

        opener::open(target)?;
        Ok(())
    }

    pub fn copy(value: &str) -> AppResult<()> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| AppError::Action(format!("Clipboard unavailable: {e}")))?;
        clipboard
            .set_text(value.to_string())
            .map_err(|e| AppError::Action(format!("Clipboard write failed: {e}")))?;
        Ok(())
    }
}

fn normalize_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}
