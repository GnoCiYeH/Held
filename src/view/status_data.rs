use held_core::view::{colors::Colors, style::CharStyle};

use crate::buffer::Buffer;

pub struct StatusLineData {
    pub content: String,
    pub color: Colors,
    pub style: CharStyle,
}

pub fn buffer_status_data(buffer: Option<&Buffer>) -> StatusLineData {
    if let Some(buffer) = buffer {
        let modified = buffer.modified();
        let (title, style) = buffer
            .relative_path()
            .as_ref()
            .map(|path| {
                if modified {
                    (format!(" {}*", path.to_string_lossy()), CharStyle::Bold)
                } else {
                    (format!(" {}", path.to_string_lossy()), CharStyle::Default)
                }
            })
            .unwrap_or_default();
        StatusLineData {
            content: title,
            color: Colors::Focused,
            style,
        }
    } else {
        StatusLineData {
            content: String::new(),
            color: Colors::Focused,
            style: CharStyle::Default,
        }
    }
}
