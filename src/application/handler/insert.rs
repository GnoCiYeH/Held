use held_core::utils::position::Position;

use crate::application::Application;
use crate::errors::*;

pub fn backspace(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        // 在第一行第一列时不执行删除操作
        if buffer.cursor.position != Position::new(0, 0) {
            buffer.cursor.move_left();
            buffer.delete();
        }
    }
    Ok(())
}
