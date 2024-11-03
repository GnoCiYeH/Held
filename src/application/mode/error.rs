use super::ModeRenderer;
use crate::{application::mode::ModeData, errors::*};
pub struct ErrorRenderer;

impl ModeRenderer for ErrorRenderer {
    fn render(
        _workspace: &mut crate::workspace::Workspace,
        _monitor: &mut crate::view::monitor::Monitor,
        mode: &mut super::ModeData,
    ) -> Result<()> {
        if let ModeData::Error(e) = mode {
            panic!("{e:?}");
        }
        todo!()
    }

    fn render_line_status(
        _workspace: &mut crate::workspace::Workspace,
        _monitor: &mut crate::view::monitor::Monitor,
        _mode: &mut ModeData,
    ) -> Result<()> {
        todo!()
    }
}
