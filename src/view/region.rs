use crate::errors::*;
use error_chain::bail;
use held_core::utils::{position::Position, rectangle::Rectangle};
use std::{
    cell::RefCell,
    fmt::Debug,
    rc::Rc,
    sync::{atomic::AtomicU16, Arc, Once},
};

use super::{render::render_buffer::CachedRenderBuffer, terminal::Terminal};

static REGION_ID_ALLOCATOR: AtomicU16 = AtomicU16::new(0);

fn alloc_region_id() -> u16 {
    REGION_ID_ALLOCATOR.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

#[derive(Debug, Clone, Copy)]
pub enum SplitMode {
    // 垂直
    Vertical,
    // 水平
    Horizon,
}

#[derive(Debug, Clone, Copy)]
pub enum SplitData {
    None,
    // 垂直
    Vertical(usize),
    // 水平
    Horizon(usize),
}

impl SplitData {
    const MIN_SCALE: usize = 10;

    pub fn zoom_down(&mut self, step: usize) {
        if let Some(scale) = match self {
            SplitData::None => None,
            SplitData::Vertical(scale) => Some(scale),
            SplitData::Horizon(scale) => Some(scale),
        } {
            *scale = (Self::MIN_SCALE).max(*scale - step);
        }
    }

    pub fn zoom_up(&mut self, step: usize) {
        if let Some(scale) = match self {
            SplitData::None => None,
            SplitData::Vertical(scale) => Some(scale),
            SplitData::Horizon(scale) => Some(scale),
        } {
            *scale = (100 - Self::MIN_SCALE).min(*scale + step);
        }
    }
}

#[derive(Debug)]
pub struct RootRegion {
    id: u16,
    split_data: SplitData,
    terminal: Arc<Box<dyn Terminal>>,
    cached_buffer: Rc<RefCell<CachedRenderBuffer>>,
    children: Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)>,
    render_line_count: bool,
}

impl RootRegion {
    pub fn new(terminal: Arc<Box<dyn Terminal>>) -> Rc<RefCell<RootRegion>> {
        static mut ROOT: Option<Rc<RefCell<RootRegion>>> = None;
        static ONCE: std::sync::Once = Once::new();
        ONCE.call_once(|| {
            unsafe {
                let cached_buffer = Rc::new(RefCell::new(CachedRenderBuffer::new(
                    terminal.width().unwrap(),
                    terminal.height().unwrap(),
                )));
                ROOT = Some(Rc::new(RefCell::new(RootRegion {
                    id: alloc_region_id(),
                    terminal,
                    children: None,
                    split_data: SplitData::None,
                    cached_buffer,
                    render_line_count: true,
                })))
            };
        });
        return unsafe { ROOT.as_ref().unwrap().clone() };
    }

    pub fn find_region_by_id(
        root: &Rc<RefCell<dyn Region>>,
        id: u16,
    ) -> Option<Rc<RefCell<dyn Region>>> {
        let root_ref = root.borrow();
        if root_ref.id() == id {
            drop(root_ref);
            return Some(root.clone());
        }

        if let Some((child1, child2)) = &root_ref.children() {
            if let Some(ans) = Self::find_region_by_id(child1, id) {
                return Some(ans);
            }

            if let Some(ans) = Self::find_region_by_id(child2, id) {
                return Some(ans);
            }
        }

        None
    }

    pub fn fill_leaf_regions(
        root: &Rc<RefCell<dyn Region>>,
        result: &mut Vec<Rc<RefCell<dyn Region>>>,
    ) {
        if let Some((child1, child2)) = &root.borrow().children() {
            Self::fill_leaf_regions(child1, result);
            Self::fill_leaf_regions(child2, result);
        } else {
            result.push(root.clone());
        }
    }
}

#[derive(Debug)]
pub struct StaticRegion {
    id: u16,
    father: u16,
    position: Position,
    width: usize,
    height: usize,
    split_data: SplitData,
    cached_buffer: Rc<RefCell<CachedRenderBuffer>>,
    children: Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)>,
    render_line_count: bool,
}

impl StaticRegion {
    pub fn new(father_id: u16, position: Position, width: usize, height: usize) -> StaticRegion {
        let ret = StaticRegion {
            id: alloc_region_id(),
            father: father_id,
            position,
            width,
            height,
            children: None,
            split_data: SplitData::None,
            cached_buffer: Rc::new(RefCell::new(CachedRenderBuffer::new(width, height))),
            render_line_count: true,
        };
        // warn!("alloc StaticRegion {}", ret.id);

        return ret;
    }
}

pub trait Region: Debug {
    fn id(&self) -> u16;

    fn father_id(&self) -> Option<u16>;

    fn position(&self, edge: bool) -> Position;

    fn width(&self, edge: bool) -> usize;

    fn height(&self, edge: bool) -> usize;

    fn set_position(&mut self, position: Position);

    fn set_width(&mut self, width: usize);

    fn set_height(&mut self, height: usize);

    fn render_line_count(&self) -> bool;

    fn set_render_line_count_enabled(&mut self, render: bool);

    // 将当前区域按比例分割为两部分，scale必须小于等于100，
    fn split(
        &mut self,
        split_mode: SplitMode,
        scale_percent: usize,
        layout_first: bool,
    ) -> (Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>) {
        let position = self.position(true);

        let (first, second): (Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>) = match split_mode {
            SplitMode::Vertical => {
                let up_size = self.height(true) * scale_percent / 100;
                let down_size = self.height(true) - up_size;

                let up = Rc::new(RefCell::new(StaticRegion::new(
                    self.id(),
                    position,
                    self.width(true),
                    up_size,
                )));

                let down = Rc::new(RefCell::new(StaticRegion::new(
                    self.id(),
                    Position {
                        line: position.line + up_size - 1,
                        offset: position.offset,
                    },
                    self.width(true),
                    down_size + 1,
                )));

                (up, down)
            }
            SplitMode::Horizon => {
                let left_size = self.width(true) * scale_percent / 100;
                let right_size = self.width(true) - left_size + 1;

                let left: Rc<RefCell<StaticRegion>> = Rc::new(RefCell::new(StaticRegion::new(
                    self.id(),
                    position,
                    left_size,
                    self.height(true),
                )));

                let right = Rc::new(RefCell::new(StaticRegion::new(
                    self.id(),
                    Position {
                        line: position.line,
                        offset: position.offset + left_size - 1,
                    },
                    right_size,
                    self.height(true),
                )));

                (left, right)
            }
        };

        if let Some(children) = self.children() {
            let (mut region, id) = if layout_first {
                let id = second.borrow().id();
                (second.borrow_mut(), id)
            } else {
                let id = first.borrow().id();
                (first.borrow_mut(), id)
            };

            children.0.borrow_mut().set_father(id);
            children.1.borrow_mut().set_father(id);

            region.set_split_data(self.split_data());
            region.set_children(Some(children));
        }

        self.set_split_data(match split_mode {
            SplitMode::Vertical => SplitData::Vertical(scale_percent),
            SplitMode::Horizon => SplitData::Horizon(scale_percent),
        });
        self.set_children(Some((first, second)));
        return self.children().unwrap();
    }

    fn set_father(&mut self, id: u16);

    fn children(&self) -> Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)>;

    fn set_children(
        &mut self,
        children: Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)>,
    );

    fn split_data(&self) -> SplitData;

    fn set_split_data(&mut self, split_data: SplitData);

    fn handle_resize(&mut self) {
        self.update_render_buffer();
        if let Some((child1, child2)) = self.children() {
            match self.split_data() {
                SplitData::None => {}
                SplitData::Vertical(scale_percent) => {
                    let up_size = self.height(true) * scale_percent / 100;
                    let down_size = self.height(true) - up_size + 1;

                    let mut up = child1.borrow_mut();
                    let mut down = child2.borrow_mut();

                    up.set_height(up_size);
                    up.set_width(self.width(true));

                    let self_position = self.position(true);

                    down.set_height(down_size);
                    down.set_width(self.width(true));
                    down.set_position(Position {
                        line: self_position.line + up_size - 1,
                        offset: self_position.offset,
                    });

                    up.handle_resize();
                    down.handle_resize();
                }
                SplitData::Horizon(scale_percent) => {
                    let left_size = self.width(true) * scale_percent / 100;
                    let right_size = self.width(true) - left_size + 1;

                    let mut left = child1.borrow_mut();
                    let mut right = child2.borrow_mut();

                    left.set_height(self.height(true));
                    left.set_width(left_size);

                    let self_position = self.position(true);

                    left.set_position(self_position);

                    right.set_height(self.height(true));
                    right.set_width(right_size);
                    right.set_position(Position {
                        line: self_position.line,
                        offset: self_position.offset + left_size - 1,
                    });

                    left.handle_resize();
                    right.handle_resize();
                }
            }
        }
    }

    fn cached_render_buffer(&self) -> Rc<RefCell<CachedRenderBuffer>>;

    fn clear_render_cache(&self) {
        self.cached_render_buffer().borrow_mut().clear_cache();
    }

    fn update_render_buffer(&self);

    fn rectangle(&self, edge: bool) -> Rectangle {
        Rectangle {
            position: self.position(edge),
            width: self.width(edge),
            height: self.height(edge),
        }
    }
}

impl Region for RootRegion {
    fn id(&self) -> u16 {
        self.id
    }

    fn father_id(&self) -> Option<u16> {
        None
    }

    fn position(&self, edge: bool) -> Position {
        if edge {
            return (0, 0).into();
        } else {
            return (1, 1).into();
        }
    }

    fn width(&self, edge: bool) -> usize {
        if edge {
            return self.terminal.width().unwrap();
        } else {
            return self.terminal.width().unwrap().saturating_sub(2);
        }
    }

    fn height(&self, edge: bool) -> usize {
        // 考虑statusLines，在这层抽象中，状态栏独立出来，不属于rootRegion
        if edge {
            return self.terminal.height().unwrap().saturating_sub(1);
        } else {
            return self.terminal.height().unwrap().saturating_sub(3);
        }
    }

    fn children(&self) -> Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)> {
        return self.children.clone();
    }

    fn set_children(
        &mut self,
        children: Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)>,
    ) {
        self.children = children;
        self.handle_resize();
    }

    fn split_data(&self) -> SplitData {
        self.split_data
    }

    fn set_split_data(&mut self, split_data: SplitData) {
        self.split_data = split_data;
    }

    fn set_position(&mut self, _position: Position) {
        unreachable!()
    }

    fn set_width(&mut self, _width: usize) {
        unreachable!()
    }

    fn set_height(&mut self, _height: usize) {
        unreachable!()
    }

    fn cached_render_buffer(&self) -> Rc<RefCell<CachedRenderBuffer>> {
        self.cached_buffer.clone()
    }

    fn update_render_buffer(&self) {
        *self.cached_render_buffer().borrow_mut() =
            CachedRenderBuffer::new(self.width(true), self.height(true) - 1);
    }

    fn set_father(&mut self, _: u16) {}

    fn render_line_count(&self) -> bool {
        self.render_line_count
    }

    fn set_render_line_count_enabled(&mut self, render: bool) {
        self.render_line_count = render;
    }
}

impl Region for StaticRegion {
    fn id(&self) -> u16 {
        self.id
    }

    fn father_id(&self) -> Option<u16> {
        return Some(self.father);
    }

    fn position(&self, edge: bool) -> Position {
        if edge {
            return self.position;
        } else {
            return self.position + Position::new(1, 1);
        }
    }

    fn width(&self, edge: bool) -> usize {
        if edge {
            return self.width;
        } else {
            return self.width.saturating_sub(2);
        }
    }

    fn height(&self, edge: bool) -> usize {
        if edge {
            return self.height;
        } else {
            return self.height.saturating_sub(2);
        }
    }

    fn children(&self) -> Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)> {
        return self.children.clone();
    }

    fn set_children(
        &mut self,
        children: Option<(Rc<RefCell<dyn Region>>, Rc<RefCell<dyn Region>>)>,
    ) {
        self.children = children;
        self.handle_resize();
    }

    fn split_data(&self) -> SplitData {
        self.split_data
    }

    fn set_split_data(&mut self, split_data: SplitData) {
        self.split_data = split_data;
    }

    fn set_position(&mut self, position: Position) {
        self.position = position;
    }

    fn set_width(&mut self, width: usize) {
        self.width = width;
    }

    fn set_height(&mut self, height: usize) {
        self.height = height;
    }

    fn cached_render_buffer(&self) -> Rc<RefCell<CachedRenderBuffer>> {
        self.cached_buffer.clone()
    }

    fn update_render_buffer(&self) {
        *self.cached_render_buffer().borrow_mut() =
            CachedRenderBuffer::new(self.width(true), self.height(true));
    }

    fn set_father(&mut self, id: u16) {
        self.father = id
    }

    fn render_line_count(&self) -> bool {
        self.render_line_count
    }

    fn set_render_line_count_enabled(&mut self, render: bool) {
        self.render_line_count = render
    }
}

impl Drop for StaticRegion {
    fn drop(&mut self) {
        // warn!("region {} dropped", self.id);
    }
}
