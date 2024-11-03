use std::sync::atomic::AtomicUsize;

use held_core::{
    utils::position::Position,
    view::{colors::Colors, style::CharStyle},
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
struct TerminalCachedBufferCell {
    char_style: CharStyle,
    colors: Colors,
    lexeme: String,
}

impl TerminalCachedBufferCell {
    pub fn compare(&self, char_style: CharStyle, colors: Colors, lexeme: &str) -> bool {
        return self.char_style == char_style && self.colors == colors && self.lexeme == lexeme;
    }
}

#[derive(Debug)]
pub struct TerminalCachedBuffer {
    height: usize,
    width: usize,
    buffer: Vec<Option<TerminalCachedBufferCell>>,
}

impl TerminalCachedBuffer {
    pub fn new(width: usize, height: usize) -> TerminalCachedBuffer {
        TerminalCachedBuffer {
            height,
            width,
            buffer: vec![None; height * width],
        }
    }

    fn compare(&self, index: usize, char_style: CharStyle, colors: Colors, lexeme: &str) -> bool {
        if index < self.buffer.len() {
            return self.buffer[index].is_some()
                && self.buffer[index]
                    .as_ref()
                    .unwrap()
                    .compare(char_style, colors, lexeme);
        }

        return false;
    }

    pub fn compare_or_update(
        &mut self,
        position: &Position,
        char_style: CharStyle,
        colors: Colors,
        content: String,
    ) -> bool {
        let mut index = position.line * self.width + position.offset;
        let mut result = true;
        for lexeme in content.graphemes(true) {
            if index < self.buffer.len() && !self.compare(index, char_style, colors, lexeme) {
                self.buffer[index] = Some(TerminalCachedBufferCell {
                    char_style,
                    colors,
                    lexeme: lexeme.into(),
                });
                result = false;
            }
            index += 1;
        }

        result
    }
}
