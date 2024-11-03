use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

use crate::encoding::Utf8Encoding;
use crate::modules::perferences::Perferences;
use crate::plugin::system::PluginSystem;
use crate::view::colors::to_rgb;
use crate::view::region::Region;
use crate::{buffer::Buffer, util::line_iterator::LineIterator, view::terminal::Terminal};
use crate::{errors::*, get_application};
use crossterm::style::Color;
use held_core::plugin::Plugin;
use held_core::utils::position::Position;
use held_core::utils::range::Range;
use held_core::utils::rectangle::Rectangle;
use held_core::view::colors::Colors;
use held_core::view::render::ContentRenderBuffer;
use held_core::view::style::CharStyle;
use syntect::highlighting::{HighlightIterator, Highlighter, Style, Theme};
use syntect::parsing::{ScopeStack, SyntaxSet};

use unicode_segmentation::UnicodeSegmentation;

use super::line_number_string_iter::LineNumberStringIter;
use super::render_buffer::Cell;
use super::render_state::RenderState;
use super::{lexeme_mapper::LexemeMapper, render_buffer::RenderBuffer};

const RENDER_CACHE_FREQUENCY: usize = 100;

pub struct Renderer<'a, 'p> {
    buffer: &'a Buffer,
    render_buffer: &'a mut RenderBuffer<'p>,
    terminal: &'a dyn Terminal,
    theme: &'a Theme,
    highlight_ranges: Option<&'a [(Range, CharStyle, Colors)]>,
    scroll_offset: usize,
    line_number_iter: LineNumberStringIter,
    content_start_of_line: usize,
    cached_render_state: &'a Rc<RefCell<HashMap<usize, RenderState>>>,
    syntax_set: &'a SyntaxSet,
    screen_position: Position,
    buffer_position: Position,
    cursor_position: Option<Position>,
    current_style: Style,
    perferences: &'a dyn Perferences,
    plugin_system: &'a mut PluginSystem,
    rectangle: Rectangle,
    render_line_count: bool,
    focused: bool,
    current_use_width: usize,
}

impl<'a, 'p> Renderer<'a, 'p> {
    pub fn new(
        buffer: &'a Buffer,
        render_buffer: &'a mut RenderBuffer<'p>,
        terminal: &'a dyn Terminal,
        perferences: &'a dyn Perferences,
        highlight_ranges: Option<&'a [(Range, CharStyle, Colors)]>,
        cached_render_state: &'a Rc<RefCell<HashMap<usize, RenderState>>>,
        theme: &'a Theme,
        syntax_set: &'a SyntaxSet,
        scroll_offset: usize,
        plugin_system: &'a mut PluginSystem,
        region: &Rc<RefCell<dyn Region>>,
        focused: bool,
    ) -> Renderer<'a, 'p> {
        let line_number_iter = LineNumberStringIter::new(buffer, scroll_offset);
        let content_start_of_line = if region.borrow().render_line_count() {
            line_number_iter.width() + 1
        } else {
            1
        };
        Self {
            buffer,
            render_buffer,
            terminal,
            theme,
            scroll_offset,
            syntax_set,
            cached_render_state,
            screen_position: Position::default(),
            buffer_position: Position::default(),
            cursor_position: None,
            current_style: Style::default(),
            perferences,
            highlight_ranges,
            line_number_iter,
            content_start_of_line,
            plugin_system,
            rectangle: region.borrow().rectangle(false),
            render_line_count: region.borrow().render_line_count(),
            focused,
            current_use_width: 0,
        }
    }

    pub fn render(
        &mut self,
        lines: LineIterator<'a>,
        mut lexeme_mapper: Option<&mut dyn LexemeMapper>,
    ) -> Result<Option<Position>> {
        self.terminal.set_cursor(None)?;
        self.render_line_number();

        let highlighter = Highlighter::new(&self.theme);
        let syntax_definition = self
            .buffer
            .syntax_definition
            .as_ref()
            .ok_or("Buffer has no syntax definition")?;

        let focused_style = Renderer::mapper_keyword_style(&highlighter);
        let blurred_style = Renderer::mapper_comment_style(&highlighter);

        let (cached_line_num, mut state) = self
            .current_cached_render_state()
            .unwrap_or((0, RenderState::new(&highlighter, syntax_definition)));

        for (line_num, line_data) in lines {
            if line_num >= cached_line_num {
                if line_num % RENDER_CACHE_FREQUENCY == 0 {
                    self.cached_render_state
                        .borrow_mut()
                        .insert(line_num, state.clone());
                }

                if self.before_visible() {
                    self.try_to_advance_to_next_line(&line_data);
                    continue;
                }

                if self.after_visible() {
                    break;
                }

                let events = state
                    .parse
                    .parse_line(&line_data, self.syntax_set)
                    .chain_err(|| "Failed to parse buffer")?;

                let styled_lexemes =
                    HighlightIterator::new(&mut state.highlight, &events, &line_data, &highlighter);

                for (style, lexeme) in styled_lexemes {
                    if let Some(ref mut mapper) = lexeme_mapper {
                        let mapped_lexemes = mapper.map(lexeme, self.buffer_position);
                        for mapped_lexeme in mapped_lexemes {
                            match mapped_lexeme {
                                super::lexeme_mapper::MappedLexeme::Focused(val) => {
                                    self.current_style = focused_style;
                                    self.render_lexeme(val.to_string());
                                }
                                super::lexeme_mapper::MappedLexeme::Blurred(val) => {
                                    self.current_style = blurred_style;
                                    self.render_lexeme(val.to_string());
                                }
                            }
                        }
                    } else {
                        self.current_style = style;
                        self.render_lexeme(lexeme);
                    }
                }
            }

            self.try_to_advance_to_next_line(&line_data);
        }

        self.render_plugins()?;
        self.render_region_edge(
            CharStyle::Bold,
            if self.focused {
                Colors::FocusedRegionEdge
            } else {
                Colors::RegionEdge
            },
        );
        Ok(self.cursor_position)
    }

    fn mapper_keyword_style(highlighter: &Highlighter) -> Style {
        highlighter.style_for_stack(
            ScopeStack::from_str("keyword")
                .unwrap_or_default()
                .as_slice(),
        )
    }

    fn render_plugins(&mut self) -> Result<()> {
        let plugin_buffers = self.plugin_system.on_render_content();
        for plugin_buffer in plugin_buffers {
            self.render_plugin(plugin_buffer)?;
        }
        Ok(())
    }

    fn render_plugin(&mut self, buffer: ContentRenderBuffer) -> Result<()> {
        let mut line = 0;
        let mut offset = 0;
        let init_pos = buffer.rectangle.position;

        for cell in buffer.cells {
            if let Some(cell) = cell {
                self.render_cell(
                    Position {
                        line: init_pos.line + line,
                        offset: init_pos.offset + offset,
                    },
                    cell.style,
                    cell.colors,
                    cell.content.to_string(),
                    true,
                );
            }

            offset += 1;
            if offset == buffer.rectangle.width {
                offset = 0;
                line += 1;
            }
        }

        Ok(())
    }

    fn mapper_comment_style(highlighter: &Highlighter) -> Style {
        highlighter.style_for_stack(
            ScopeStack::from_str("keyword")
                .unwrap_or_default()
                .as_slice(),
        )
    }

    fn current_cached_render_state(&self) -> Option<(usize, RenderState)> {
        self.cached_render_state
            .borrow()
            .iter()
            .filter(|(k, _)| **k < self.scroll_offset)
            .max_by(|a, b| a.0.cmp(b.0))
            .map(|x| (*x.0, x.1.clone()))
    }

    fn after_visible(&self) -> bool {
        self.screen_position.line >= (self.rectangle.height)
    }

    fn before_visible(&self) -> bool {
        self.buffer_position.line < self.scroll_offset
    }

    fn inside_visible(&self) -> bool {
        !self.before_visible() && !self.after_visible()
    }

    fn absolute_screen_position(&self) -> Position {
        Position {
            line: self.screen_position.line + self.rectangle.position.line,
            offset: self.screen_position.offset + self.rectangle.position.offset,
        }
    }

    fn set_cursor(&mut self) {
        if self.inside_visible() && *self.buffer.cursor == self.buffer_position {
            let position = self.absolute_screen_position();
            self.cursor_position = Some(position);
            get_application().state_data.cursor_state.screen_position = position;
        }
    }

    fn on_cursor_line(&self) -> bool {
        self.buffer.cursor.line == self.buffer_position.line
    }

    fn try_to_advance_to_next_line(&mut self, line: &str) {
        if line.chars().last().map(|x| x == '\n').unwrap_or(false) {
            self.advance_to_next_line();
        }
    }

    fn advance_to_next_line(&mut self) {
        if self.inside_visible() {
            self.set_cursor();
            self.render_rest_of_line();
            self.screen_position.line += 1;
            if !self.render_line_count {
                self.screen_position.offset = self.content_start_of_line;
                self.current_use_width = self.content_start_of_line;
            }
        }

        self.buffer_position.line += 1;
        self.buffer_position.offset = 0;
        self.render_line_number();
    }

    fn render_rest_of_line(&mut self) {
        let on_cursor_line = self.on_cursor_line();
        for offset in self.screen_position.offset
            ..(self.rectangle.width + self.screen_position.offset - self.current_use_width)
        {
            let colors = if on_cursor_line {
                Colors::Focused
            } else {
                Colors::Default
            };

            self.render_cell(
                Position {
                    line: self.screen_position.line,
                    offset,
                },
                CharStyle::Default,
                colors,
                " ",
                true,
            );
        }
    }

    fn render_line_number(&mut self) {
        if !self.inside_visible() || !self.render_line_count {
            return;
        }
        let line_number = self.line_number_iter.next().unwrap();
        let is_on_cursor_line = self.on_cursor_line();

        let style = if is_on_cursor_line {
            CharStyle::Bold
        } else {
            CharStyle::Default
        };
        // 渲染行号
        self.render_cell(
            Position {
                line: self.screen_position.line,
                offset: 0,
            },
            style,
            Colors::Focused,
            line_number,
            true,
        );

        // 行号后的gap
        let gap_color = if is_on_cursor_line {
            Colors::Focused
        } else {
            Colors::Default
        };
        self.render_cell(
            Position {
                line: self.screen_position.line,
                offset: self.line_number_iter.width(),
            },
            style,
            gap_color,
            " ",
            true,
        );

        self.screen_position.offset = self.line_number_iter.width() + 1;
        self.current_use_width = self.line_number_iter.width() + 1;
    }

    fn render_lexeme<T: Into<Cow<'a, str>>>(&mut self, lexeme: T) {
        let lexeme: Cow<'a, str> = lexeme.into();
        for character in lexeme.graphemes(true) {
            if character == "\n" {
                continue;
            }

            self.set_cursor();

            let token_color = to_rgb(self.current_style.foreground);
            let (style, color) = self.current_char_style(token_color);

            if self.perferences.line_wrapping()
                && self.current_use_width == self.rectangle.width - 1
            {
                self.render_cell(
                    self.screen_position,
                    style,
                    color,
                    character.to_string(),
                    true,
                );
                self.buffer_position.offset += 1;

                // 屏幕上换行但是渲染原来的line
                let prefix_len = self.content_start_of_line;
                let prefix = " ".repeat(prefix_len);
                self.screen_position.offset = 0;
                self.screen_position.line += 1;

                self.render_cell(self.screen_position, style, Colors::Default, prefix, true);
                self.screen_position.offset += prefix_len;
                self.current_use_width = prefix_len;
            } else if character == "\t" {
                let tab_len = self.perferences.tab_width();
                let width = tab_len - (self.screen_position.offset + 1) % tab_len;
                let tab_str = " ".repeat(width);
                self.render_lexeme(tab_str);
            } else {
                self.render_cell(
                    self.screen_position,
                    style,
                    color,
                    character.to_string(),
                    true,
                );
                self.screen_position.offset += 1;
                self.buffer_position.offset += 1;
                self.current_use_width += Utf8Encoding::width(character);
            }

            // 退出循环前更新
            self.set_cursor();
        }
    }

    fn render_cell<C: Into<Cow<'p, str>>>(
        &mut self,
        position: Position,
        style: CharStyle,
        colors: Colors,
        content: C,
        relative: bool,
    ) {
        let position = if relative {
            position + Position::new(1, 1)
        } else {
            position
        };
        self.render_buffer.set_cell(
            position,
            Cell {
                content: content.into(),
                colors,
                style,
            },
        );
    }

    pub fn render_region_edge(&mut self, style: CharStyle, colors: Colors) {
        for offset in 0..self.rectangle.width + 2 {
            self.render_cell((0, offset).into(), style, colors, " ", false);
            self.render_cell(
                (self.rectangle.height + 1, offset).into(),
                style,
                colors,
                " ",
                false,
            );
        }

        for line in 0..self.rectangle.height + 2 {
            self.render_cell((line, 0).into(), style, colors, " ", false);
            self.render_cell(
                (line, self.rectangle.width + 1).into(),
                style,
                colors,
                " ",
                false,
            );
        }

        if let Some(ref path) = self.buffer.relative_path() {
            self.render_cell(
                (0, 1).into(),
                style,
                colors,
                format!("{}", path.to_str().unwrap_or_default()),
                false,
            );
        }
    }

    fn current_char_style(&self, token_color: Color) -> (CharStyle, Colors) {
        let (style, colors) = match self.highlight_ranges {
            Some(highlight_ranges) => {
                for (range, style, colors) in highlight_ranges {
                    if range.includes(&self.buffer_position) {
                        // 修正背景色
                        let fix_colors = if let Colors::CustomForeground(color) = colors {
                            if self.on_cursor_line() {
                                Colors::CustomFocusedForeground(*color)
                            } else {
                                *colors
                            }
                        } else {
                            *colors
                        };
                        return (*style, fix_colors);
                    }
                }

                // We aren't inside one of the highlighted areas.
                // Fall back to other styling considerations.
                if self.on_cursor_line() {
                    (
                        CharStyle::Default,
                        Colors::CustomFocusedForeground(token_color),
                    )
                } else {
                    (CharStyle::Default, Colors::CustomForeground(token_color))
                }
            }
            None => {
                if self.on_cursor_line() {
                    (
                        CharStyle::Default,
                        Colors::CustomFocusedForeground(token_color),
                    )
                } else {
                    (CharStyle::Default, Colors::CustomForeground(token_color))
                }
            }
        };

        (style, colors)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        collections::HashMap,
        io::{BufReader, Cursor},
        path::Path,
        rc::Rc,
    };

    use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

    use crate::{
        buffer::Buffer,
        modules::perferences::DummyPerferences,
        util::line_iterator::LineIterator,
        view::{
            colors::map::ColorMap,
            render::render_buffer::{CachedRenderBuffer, RenderBuffer},
            terminal::{cross_terminal::CrossTerminal, Terminal},
        },
    };

    use super::Renderer;

    #[test]
    fn test_display() {
        let terminal = CrossTerminal::new().unwrap();
        let mut buffer = Buffer::from_file(Path::new("src/main.rs"), None).unwrap();
        let mut render_buffer = RenderBuffer::new(
            terminal.width().unwrap(),
            terminal.height().unwrap(),
            Rc::new(RefCell::new(CachedRenderBuffer::new(
                terminal.width().unwrap(),
                terminal.height().unwrap(),
            ))),
        );
        let perferences = DummyPerferences;
        let cached_render_state = Rc::new(RefCell::new(HashMap::new()));

        let mut reader = BufReader::new(Cursor::new(include_str!(
            "../../themes/solarized_dark.tmTheme"
        )));
        let theme = ThemeSet::load_from_reader(&mut reader).unwrap();
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let definition = buffer
            .file_extension()
            .and_then(|ex| syntax_set.find_syntax_by_extension(&ex))
            .or_else(|| Some(syntax_set.find_syntax_plain_text()))
            .cloned();

        buffer.syntax_definition = definition;
        let binding = buffer.data();
        {
            let mut renderer = Renderer::new(
                &buffer,
                &mut render_buffer,
                &terminal,
                &perferences,
                None,
                &cached_render_state,
                &theme,
                &syntax_set,
                0,
                todo!(),
                todo!(),
                todo!(),
            );
            renderer.render(LineIterator::new(&binding), None).unwrap();
        }

        for (position, cell) in render_buffer.iter() {
            terminal
                .print(
                    &position,
                    cell.style,
                    theme.map_colors(cell.colors),
                    &cell.content,
                )
                .unwrap();
        }

        terminal.present().unwrap();
    }
}
