use held_core::view::{colors::Colors, style::CharStyle};

use super::ModeRenderer;
use crate::{
    buffer::Buffer,
    errors::*,
    view::status_data::{buffer_status_data, StatusLineData},
};

pub(super) struct NormalRenderer;

impl NormalRenderer {
    pub fn render_all(
        workspace: &mut crate::workspace::Workspace,
        monitor: &mut crate::view::monitor::Monitor,
    ) -> Result<()> {
        let mut buffer_opt: Option<&Buffer> = None;
        for buffer in workspace.get_observation_buffers_iter() {
            if Some(buffer.id) == workspace.current_buffer().map(|x| x.id) {
                buffer_opt = Some(buffer);
                continue;
            }
            let mut presenter = monitor.build_presenter_by_buffer(buffer)?;
            let data = buffer.data();
            presenter.print_buffer(buffer, &data, &workspace.syntax_set, None, None)?;
            presenter.present(false)?;
        }

        // 最后渲染主buffer
        if let Some(buffer) = buffer_opt {
            let mut presenter = monitor.build_presenter_by_buffer(buffer)?;
            let data = buffer.data();
            presenter.print_buffer(buffer, &data, &workspace.syntax_set, None, None)?;
            presenter.present(true)?;
        }
        Ok(())
    }
}

impl ModeRenderer for NormalRenderer {
    fn render(
        workspace: &mut crate::workspace::Workspace,
        monitor: &mut crate::view::monitor::Monitor,
        _mode: &mut super::ModeData,
    ) -> Result<()> {
        if let Some(buffer) = workspace.current_buffer() {
            let mut presenter = monitor.build_presenter_by_buffer(buffer)?;
            let data = buffer.data();
            presenter.print_buffer(buffer, &data, &workspace.syntax_set, None, None)?;
            presenter.present(true)?;
        }
        Ok(())
    }

    fn render_line_status(
        workspace: &mut crate::workspace::Workspace,
        monitor: &mut crate::view::monitor::Monitor,
        _mode: &mut super::ModeData,
    ) -> Result<()> {
        let mode_name_data = StatusLineData {
            content: " NORMAL ".to_string(),
            color: Colors::Inverted,
            style: CharStyle::Bold,
        };
        let status_datas = &[
            mode_name_data,
            buffer_status_data(workspace.current_buffer()),
        ];
        monitor.present_status_line(status_datas)?;
        Ok(())
    }
}
