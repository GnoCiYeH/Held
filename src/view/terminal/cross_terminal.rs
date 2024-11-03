use std::{
    cell::{RefCell, RefMut},
    io::{stdout, Write},
};

use crossterm::{
    event::Event,
    terminal::{self, disable_raw_mode},
    QueueableCommand,
};
use held_core::{
    utils::{distance::Distance, position::Position},
    view::{colors::Colors, style::CharStyle},
};
use unicode_segmentation::UnicodeSegmentation;

use super::{
    terminal_cache_buffer::TerminalCachedBuffer, Terminal, MIN_HEIGHT, MIN_WIDTH,
    TERMINAL_EXECUTE_ERROR,
};
use crate::errors::*;

#[derive(Debug)]
pub struct CrossTerminal {
    ansi_buffer: RefCell<Vec<u8>>,
    current_position: RefCell<Option<Position>>,
    cache: RefCell<TerminalCachedBuffer>,
}

unsafe impl Send for CrossTerminal {}
unsafe impl Sync for CrossTerminal {}

impl CrossTerminal {
    pub fn new() -> Result<CrossTerminal> {
        crossterm::terminal::enable_raw_mode()?;
        let (width, height) = crossterm::terminal::size()?;
        let terminal = CrossTerminal {
            ansi_buffer: RefCell::default(),
            current_position: RefCell::default(),
            cache: RefCell::new(TerminalCachedBuffer::new(width as usize, height as usize)),
        };
        terminal.clear()?;
        terminal.present()?;
        Ok(terminal)
    }

    fn buffer(&self) -> RefMut<Vec<u8>> {
        return self.ansi_buffer.borrow_mut();
    }

    fn update_style(&self, char_style: CharStyle, colors: Colors) -> Result<()> {
        self.buffer().queue(crossterm::style::SetAttribute(
            crossterm::style::Attribute::Reset,
        ))?;

        match char_style {
            CharStyle::Default => {
                self.buffer().queue(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Reset,
                ))?;
            }
            CharStyle::Bold => {
                self.buffer().queue(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Bold,
                ))?;
            }
            CharStyle::Reverse => {
                self.buffer().queue(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Reverse,
                ))?;
            }
            CharStyle::Italic => {
                self.buffer().queue(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Italic,
                ))?;
            }
        }

        match colors {
            Colors::Default => {
                self.buffer().queue(crossterm::style::ResetColor)?;
            }
            Colors::CustomForeground(color) => {
                self.buffer()
                    .queue(crossterm::style::SetForegroundColor(color))?;
            }
            Colors::Custom(fg, bg) => {
                self.buffer()
                    .queue(crossterm::style::SetForegroundColor(fg))?;
                self.buffer()
                    .queue(crossterm::style::SetBackgroundColor(bg))?;
            }
            _ => {
                unreachable!();
            }
        }

        Ok(())
    }
}

impl Terminal for CrossTerminal {
    fn listen(&self) -> Result<Event> {
        crossterm::event::read().chain_err(|| "Handle event io error")
    }

    fn clear(&self) -> Result<()> {
        self.buffer()
            .queue(crossterm::style::SetAttribute(
                crossterm::style::Attribute::Reset,
            ))
            .chain_err(|| TERMINAL_EXECUTE_ERROR)
            .map(|_| ())?;
        self.buffer()
            .queue(terminal::Clear(terminal::ClearType::All))
            .chain_err(|| TERMINAL_EXECUTE_ERROR)
            .map(|_| ())?;
        Ok(())
    }

    fn present(&self) -> Result<()> {
        stdout().write_all(&self.buffer())?;
        stdout().flush()?;
        self.buffer().clear();
        Ok(())
    }

    fn width(&self) -> Result<usize> {
        let (width, _) = crossterm::terminal::size()?;
        return Ok(width.max(MIN_WIDTH).into());
    }

    fn height(&self) -> Result<usize> {
        let (_, height) = crossterm::terminal::size()?;
        return Ok(height.max(MIN_HEIGHT).into());
    }

    fn set_cursor(&self, position: Option<Position>) -> Result<()> {
        match position {
            Some(position) => {
                self.buffer()
                    .queue(crossterm::cursor::MoveTo(
                        position.offset as u16,
                        position.line as u16,
                    ))
                    .unwrap()
                    .queue(crossterm::cursor::Show)
                    .chain_err(|| TERMINAL_EXECUTE_ERROR)?;
            }
            None => {
                self.buffer()
                    .queue(crossterm::cursor::Hide)
                    .chain_err(|| TERMINAL_EXECUTE_ERROR)?;
            }
        }

        Ok(())
    }

    fn set_cursor_type(&self, cursor_type: crossterm::cursor::SetCursorStyle) -> Result<()> {
        self.buffer()
            .queue(cursor_type)
            .chain_err(|| TERMINAL_EXECUTE_ERROR)
            .map(|_| ())
    }

    fn print(
        &self,
        position: &Position,
        char_style: CharStyle,
        colors: Colors,
        content: &str,
    ) -> Result<()> {
        // if self
        //     .cache
        //     .borrow_mut()
        //     .compare_or_update(position, char_style, colors, content.into())
        // {
        //     return Ok(());
        // }
        self.update_style(char_style, colors)?;
        if *self.current_position.borrow() != Some(*position) {
            self.set_cursor(Some(*position))?;
        }

        *self.current_position.borrow_mut() = Some(
            *position
                + Distance {
                    lines: 0,
                    offset: content.graphemes(true).count(),
                },
        );
        self.buffer().queue(crossterm::style::Print(content))?;
        Ok(())
    }

    fn suspend(&self) {
        let _ = self.clear();
        let _ = self.set_cursor(Some(Position::from((0, 0))));
        let _ = stdout().write_all(&self.buffer());
        let _ = stdout().flush();

        self.buffer().clear();
    }
}

impl Drop for CrossTerminal {
    fn drop(&mut self) {
        // self.suspend();
        let _ = disable_raw_mode();
    }
}
