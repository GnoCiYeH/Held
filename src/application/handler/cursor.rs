use crate::application::Application;
use crate::errors::*;

pub fn move_left(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.cursor.move_left();
        app.monitor.scroll_to_cursor(buffer)?;
    }
    Ok(())
}

pub fn move_right(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.cursor.move_right();
        app.monitor.scroll_to_cursor(buffer)?;
    }
    Ok(())
}

pub fn move_up(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.cursor.move_up();
        app.monitor.scroll_to_cursor(buffer)?;
    }
    Ok(())
}

pub fn move_down(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.cursor.move_down();
        app.monitor.scroll_to_cursor(buffer)?;
    }
    Ok(())
}

pub fn move_to_start_of_line(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.cursor.move_to_start_of_line();
    }
    Ok(())
}
