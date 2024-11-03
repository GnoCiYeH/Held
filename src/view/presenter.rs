use std::{borrow::Cow, cell::RefCell, fmt::Debug, rc::Rc};

use super::{
    colors::map::ColorMap,
    monitor::Monitor,
    region::Region,
    render::{
        lexeme_mapper::LexemeMapper,
        render_buffer::{Cell, RenderBuffer},
    },
};
use crate::{
    buffer::Buffer, errors::*, util::line_iterator::LineIterator, view::render::renderer::Renderer,
};
use held_core::{
    utils::{position::Position, range::Range},
    view::{colors::Colors, style::CharStyle},
};
use syntect::{highlighting::Theme, parsing::SyntaxSet};

pub struct Presenter<'a> {
    view: &'a mut Monitor,
    theme: Theme,
    present_buffer: RenderBuffer<'a>,
    cursor_position: Option<Position>,
    region: Rc<RefCell<dyn Region>>,
    focused: bool,
}

impl<'a> Presenter<'a> {
    pub fn new(
        monitor: &mut Monitor,
        region: Rc<RefCell<dyn Region>>,
        focused: bool,
    ) -> Result<Presenter> {
        let theme_name = monitor.perference.borrow().theme_name();
        let mut theme = monitor
            .first_theme()
            .ok_or_else(|| format!("Couldn't find anyone theme"))?;
        if let Some(theme_name) = theme_name {
            theme = monitor
                .get_theme(&theme_name)
                .ok_or_else(|| format!("Couldn't find \"{}\" theme", theme_name))?;
        }
        let present_buffer = RenderBuffer::new(
            region.borrow().width(true),
            region.borrow().height(true),
            region.borrow().cached_render_buffer(),
        );
        Ok(Presenter {
            view: monitor,
            theme,
            present_buffer,
            cursor_position: None,
            region,
            focused,
        })
    }

    pub fn set_cursor(&mut self, position: Position) {
        self.cursor_position = Some(position);
    }

    pub fn present(&self, set_cursor: bool) -> Result<()> {
        if self.region.borrow().children().is_some() {
            // 对于非根节点不允许渲染
            return Ok(());
        }
        for (position, cell) in self.present_buffer.iter() {
            // 对于非边界的cell，需要对坐标进行修正
            self.view.terminal.print(
                &(position + self.region.borrow().rectangle(true).position),
                cell.style,
                self.theme.map_colors(cell.colors),
                &cell.content,
            )?
        }

        if set_cursor {
            self.view.terminal.set_cursor(self.cursor_position)?;
        }

        self.view.terminal.present()?;
        Ok(())
    }

    // 按照预设渲染buffer
    pub fn print_buffer(
        &mut self,
        buffer: &Buffer,
        buffer_data: &'a str,
        syntax_set: &'a SyntaxSet,
        highlights: Option<&'a [(Range, CharStyle, Colors)]>,
        lexeme_mapper: Option<&'a mut dyn LexemeMapper>,
    ) -> Result<()> {
        if self.region.borrow().children().is_some() {
            // 对于非根节点不允许渲染
            return Ok(());
        }
        let scroll_offset = self.view.get_region_controller(buffer).line_offset();
        let lines = LineIterator::new(&buffer_data);

        let cursor_position = Renderer::new(
            buffer,
            &mut self.present_buffer,
            &**self.view.terminal,
            &*self.view.perference.borrow(),
            highlights,
            self.view.get_render_cache(buffer),
            &self.theme,
            syntax_set,
            scroll_offset,
            &mut self.view.plugin_system.borrow_mut(),
            &self.region,
            self.focused,
        )
        .render(lines, lexeme_mapper)?;

        match cursor_position {
            Some(position) => self.set_cursor(position),
            None => self.cursor_position = None,
        }

        Ok(())
    }

    pub fn print<C>(&mut self, position: &Position, style: CharStyle, colors: Colors, content: C)
    where
        C: Into<Cow<'a, str>> + Debug,
    {
        if self.region.borrow().children().is_some() {
            // 对于非根节点不允许渲染
            return;
        }
        self.present_buffer.set_cell(
            *position,
            Cell {
                content: content.into(),
                style,
                colors,
            },
        );
    }
}
