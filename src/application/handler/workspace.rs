use crate::application::mode::{ModeData, ModeKey, ModeRenderer, ModeRouter};
use crate::application::Application;
use crate::errors::*;

pub fn save_file(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.save()?;
    }
    Ok(())
}

pub fn undo(app: &mut Application) -> Result<()> {
    if let Some(buffer) = app.workspace.current_buffer_mut() {
        buffer.undo();
    }
    Ok(())
}

pub fn to_normal_mode(app: &mut Application) -> Result<()> {
    if let ModeData::Workspace(ref mode) = app.mode {
        app.workspace
            .change_to_target_observation_buffer(mode.prev_buffer_id, &mut app.monitor)?;
    }
    app.switch_mode(ModeKey::Normal)?;
    Ok(())
}

pub fn close_dir_tree(app: &mut Application) -> Result<()> {
    if let ModeData::Workspace(ref mut mode) = app.mode {
        mode.close(&mut app.workspace, &mut app.monitor)?;
    }
    app.switch_mode(ModeKey::Normal)?;
    Ok(())
}

pub fn move_down(app: &mut Application) -> Result<()> {
    if let ModeData::Workspace(ref mut mode) = app.mode {
        mode.move_down();
    }
    Ok(())
}

pub fn move_up(app: &mut Application) -> Result<()> {
    if let ModeData::Workspace(ref mut mode) = app.mode {
        mode.move_up();
    }
    Ok(())
}

pub fn enter(app: &mut Application) -> Result<()> {
    if let ModeData::Workspace(ref mut mode) = app.mode {
        if mode.open(&mut app.workspace, &mut app.monitor)? {
            to_normal_mode(app)?;
        }
    }
    Ok(())
}

pub fn open_with_vertical(app: &mut Application) -> Result<()> {
    if let ModeData::Workspace(ref mut mode) = app.mode {
        mode.open_with_split(
            &mut app.workspace,
            &mut app.monitor,
            crate::view::region::SplitMode::Vertical,
        )?;
        to_normal_mode(app)?;
    }
    Ok(())
}

pub fn open_with_horizon(app: &mut Application) -> Result<()> {
    if let ModeData::Workspace(ref mut mode) = app.mode {
        mode.open_with_split(
            &mut app.workspace,
            &mut app.monitor,
            crate::view::region::SplitMode::Horizon,
        )?;
        to_normal_mode(app)?;
    }
    Ok(())
}

pub fn next_observation_buffer(app: &mut Application) -> Result<()> {
    app.workspace
        .change_to_next_observation_buffer(&mut app.monitor)?;
    ModeRouter::render_line_status(&mut app.workspace, &mut app.monitor, &mut app.mode)?;
    Ok(())
}
