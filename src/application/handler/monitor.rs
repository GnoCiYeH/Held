use crate::application::Application;
use crate::errors::*;

pub fn scroll_to_cursor(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer() {
        app.monitor.scroll_to_cursor(buffer)?;
    }
    Ok(())
}

pub fn scroll_to_center(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer() {
        app.monitor.scroll_to_center(buffer)?;
    }
    Ok(())
}

pub fn zoom_up(app: &mut Application) -> Result<()> {
    app.monitor.zoom_up(1);
    Ok(())
}

pub fn zoom_down(app: &mut Application) -> Result<()> {
    app.monitor.zoom_down(1);
    Ok(())
}
