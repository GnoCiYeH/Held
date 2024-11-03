use unicode_segmentation::UnicodeSegmentation;

use crate::buffer::Buffer;
use crate::errors::*;
use crate::view::region::Region;
use crate::view::render::line_number_string_iter::LineNumberStringIter;
use std::cell::RefCell;
use std::rc::Rc;

/// 对于滚动操作的抽象对象
///
/// 外部通过line_offset方法获取滚动后buffer的offset
pub struct RegionController {
    region: Rc<RefCell<dyn Region>>,
    line_offset: usize,
}

impl RegionController {
    pub fn new(region: Rc<RefCell<dyn Region>>, init_line_index: usize) -> RegionController {
        RegionController {
            region,
            line_offset: init_line_index,
        }
    }

    pub fn set_region(&mut self, region: Rc<RefCell<dyn Region>>) {
        self.region = region;
    }

    pub fn region(&self) -> &Rc<RefCell<dyn Region>> {
        &self.region
    }

    // 若将buffer指针指向的行滚动到显示区域顶部
    pub fn scroll_into_monitor(&mut self, buffer: &Buffer) -> Result<()> {
        let terminal_height = self.region.borrow().height(false);
        if self.line_offset > buffer.cursor.line {
            self.line_offset = buffer.cursor.line;
        } else {
            let gutter_width = LineNumberStringIter::new(buffer, 0).width() + 1;
            let buffer_content_width = self.region.borrow().width(false) - gutter_width;
            let wappered_line_height = buffer
                .data()
                .lines()
                .skip(self.line_offset)
                .take(terminal_height)
                .map(|line| {
                    let grapheme_count = line.graphemes(true).count().max(1);
                    ((grapheme_count + buffer_content_width - 1) / buffer_content_width).max(1)
                })
                .collect::<Vec<_>>();

            let wappered_line_count: usize = wappered_line_height.iter().sum();

            if self.line_offset + terminal_height + terminal_height
                <= buffer.cursor.line + wappered_line_count
            {
                self.line_offset += 1;
            }
        }

        Ok(())
    }

    // 将buffer指针指向的行滚动到显示区域中间区域
    pub fn scroll_to_center(&mut self, buffer: &Buffer) -> Result<()> {
        self.line_offset = buffer
            .cursor
            .line
            .saturating_sub(self.region.borrow().height(false).saturating_div(2));
        Ok(())
    }

    // 向上滚动n行
    pub fn scroll_up(&mut self, line_count: usize) {
        self.line_offset = self.line_offset.saturating_sub(line_count);
    }

    // 向下滚动n行
    pub fn scroll_down(&mut self, line_count: usize) {
        self.line_offset = self.line_offset.saturating_add(line_count);
    }

    // 返回当前的offset
    pub fn line_offset(&self) -> usize {
        self.line_offset
    }
}
