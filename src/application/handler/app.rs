use crate::application::mode::ModeKey;
use crate::application::Application;
use crate::errors::*;

pub fn exit(app: &mut Application) -> Result<()> {
    app.switch_mode(ModeKey::Exit)?;
    Ok(())
}

pub fn to_insert_mode(app: &mut Application) -> Result<()> {
    app.switch_mode(ModeKey::Insert)?;
    Ok(())
}

pub fn to_normal_mode(app: &mut Application) -> Result<()> {
    app.switch_mode(ModeKey::Normal)?;
    Ok(())
}

pub fn to_workspace_mode(app: &mut Application) -> Result<()> {
    app.switch_mode(ModeKey::Workspace)?;
    Ok(())
}
