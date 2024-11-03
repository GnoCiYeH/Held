use std::{borrow::Cow, cell::RefCell, rc::Rc};

use held_core::view::{colors::Colors, style::CharStyle};
use unicode_segmentation::UnicodeSegmentation;

use held_core::utils::position::Position;

#[derive(Debug, Clone, PartialEq)]
pub struct Cell<'a> {
    pub content: Cow<'a, str>,
    pub colors: Colors,
    pub style: CharStyle,
}

impl<'c> Default for Cell<'c> {
    fn default() -> Self {
        Self {
            content: " ".into(),
            colors: Default::default(),
            style: Default::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CachedCell {
    pub content: String,
    pub colors: Colors,
    pub style: CharStyle,
}

#[derive(Debug)]
pub struct CachedRenderBuffer {
    pub cells: Vec<Option<CachedCell>>,
}

impl CachedRenderBuffer {
    pub fn new(width: usize, height: usize) -> CachedRenderBuffer {
        CachedRenderBuffer {
            cells: vec![None; width * height],
        }
    }

    pub fn clear_cache(&mut self) {
        self.cells.fill(None);
    }

    // 返回对应index是否与cell相等
    pub fn compare_and_update(&mut self, cell: &Cell, index: usize) -> bool {
        if index < self.cells.len() {
            let cache = &mut self.cells[index];
            let cell_content = String::from_iter(cell.content.chars());

            let equal = if let Some(cache) = cache {
                let equal = cache.colors == cell.colors
                    && cache.style == cell.style
                    && cache.content == cell_content;

                equal
            } else {
                false
            };

            if !equal {
                let mut cache_cell = CachedCell::default();
                let content_len = cell_content.graphemes(true).count();
                cache_cell.colors = cell.colors;
                cache_cell.style = cell.style;
                cache_cell.content = cell_content;
                for i in (index + 1)..(index + content_len) {
                    if i < self.cells.len() {
                        self.cells[i] = None
                    }
                }

                self.cells[index] = Some(cache_cell);
            }

            return equal;
        } else {
            return false;
        }
    }
}

#[derive(Debug)]
pub struct RenderBuffer<'a> {
    width: usize,
    height: usize,
    cells: Vec<Cell<'a>>,
    cached: Rc<RefCell<CachedRenderBuffer>>,
}

impl<'a> RenderBuffer<'a> {
    pub fn new(
        width: usize,
        height: usize,
        cached: Rc<RefCell<CachedRenderBuffer>>,
    ) -> RenderBuffer<'a> {
        RenderBuffer {
            width,
            height,
            cells: vec![Cell::default(); width * height],
            cached,
        }
    }

    pub fn set_cell(&mut self, position: Position, cell: Cell<'a>) {
        if position.line >= self.height || position.offset >= self.width {
            return;
        }
        let range = (position.line * self.width)
            ..(position.line * self.width + self.width).min(self.cells.len() - 1);

        let index = position.line * self.width + position.offset;
        if range.contains(&index) {
            self.cells[index] = cell;
        }
        // if index < self.cells.len() {
        //     self.cells[index] = cell;
        // }
    }

    pub fn clear(&mut self) {
        self.cells = vec![Cell::default(); self.width * self.height];
    }

    pub fn iter(&self) -> RenderBufferIter {
        RenderBufferIter::new(self)
    }
}

pub struct RenderBufferIter<'a> {
    index: usize,
    width: usize,
    cells: &'a Vec<Cell<'a>>,
    cached: Rc<RefCell<CachedRenderBuffer>>,
}

impl<'a> RenderBufferIter<'a> {
    pub fn new(render_buffer: &'a RenderBuffer) -> RenderBufferIter<'a> {
        RenderBufferIter {
            index: 0,
            width: render_buffer.width,
            cells: &render_buffer.cells,
            cached: render_buffer.cached.clone(),
        }
    }
}

impl<'a> Iterator for RenderBufferIter<'a> {
    type Item = (Position, &'a Cell<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.cells.len() {
            let position = Position {
                line: self.index / self.width,
                offset: self.index % self.width,
            };

            let index = self.index;
            let cell = &self.cells[self.index];
            self.index += cell.content.graphemes(true).count().max(1);

            // if !self.cached.borrow_mut().compare_and_update(cell, index) {
            //     return Some((position, cell));
            // }
            return Some((position, cell));
        }
        None
    }
}
