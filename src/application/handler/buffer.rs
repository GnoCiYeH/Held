use crossterm::event::KeyCode;
use held_core::utils::position::Position;

use crate::application::Application;
use crate::errors::*;

use super::cursor;

pub fn insert_char(app: &mut Application) -> Result<()> {
    if let Some(key) = app.monitor.last_key {
        if let KeyCode::Char(c) = key.code {
            app.workspace.current_buffer_mut().unwrap().insert(c);
            cursor::move_right(app)?;
        }
    }
    Ok(())
}

pub fn new_line(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.insert('\n');
    }
    Ok(())
}

pub fn insert_tab(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        let tab_len = app.perferences.borrow().tab_width();
        let width = tab_len - (buffer.cursor.offset) % tab_len;
        if app.perferences.borrow().soft_tab() {
            let tab_str = " ".repeat(width);
            buffer.insert(tab_str);
            buffer.cursor.move_to(Position {
                line: buffer.cursor.line,
                offset: buffer.cursor.offset + width,
            });
        } else {
            buffer.insert("\t");
            buffer.cursor.move_to(Position {
                line: buffer.cursor.line,
                offset: buffer.cursor.offset + width,
            });
        }
    }
    Ok(())
}
