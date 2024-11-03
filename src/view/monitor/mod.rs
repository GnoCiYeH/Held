use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use super::{
    colors::map::ColorMap,
    presenter::Presenter,
    region::{Region, RootRegion, SplitMode},
    render::{render_buffer::CachedRenderBuffer, render_state::RenderState},
    status_data::StatusLineData,
    terminal::{cross_terminal::CrossTerminal, Terminal},
    theme_loadler::ThemeLoader,
};
use crate::errors::*;
use crate::modules::perferences::Perferences;
use crate::ErrorKind::*;
use crate::{buffer::Buffer, plugin::system::PluginSystem};
use crossterm::event::{Event, KeyEvent};
use region_controller::RegionController;
use syntect::highlighting::{Theme, ThemeSet};

pub mod region_controller;

/// 管理所有的显示
pub struct Monitor {
    pub terminal: Arc<Box<dyn Terminal>>,
    theme_set: ThemeSet,
    pub perference: Rc<RefCell<dyn Perferences>>,
    region_controllers: HashMap<usize, RegionController>,
    render_caches: HashMap<usize, Rc<RefCell<HashMap<usize, RenderState>>>>,
    pub last_key: Option<KeyEvent>,
    pub plugin_system: Rc<RefCell<PluginSystem>>,
    pub root_region: Rc<RefCell<RootRegion>>,
    selected_region_index: usize,
    leaf_regions: Vec<Rc<RefCell<dyn Region>>>,
    regions: HashMap<u16, Rc<RefCell<dyn Region>>>,
    status_line_cached_buffer: Rc<RefCell<CachedRenderBuffer>>,
}

impl Monitor {
    pub fn new(
        perference: Rc<RefCell<dyn Perferences>>,
        plugin_system: Rc<RefCell<PluginSystem>>,
    ) -> Result<Monitor> {
        let terminal: Arc<Box<dyn Terminal>> = Arc::new(Box::new(CrossTerminal::new()?));
        let theme_set = ThemeLoader::new(perference.borrow().theme_path()?).load()?;
        let root_region = RootRegion::new(terminal.clone());
        let mut leaf_regions = Vec::new();
        RootRegion::fill_leaf_regions(
            &(root_region.clone() as Rc<RefCell<dyn Region>>),
            &mut leaf_regions,
        );

        let mut regions: HashMap<u16, Rc<RefCell<dyn Region>>> = HashMap::new();
        regions.insert(root_region.borrow().id(), root_region.clone());

        let status_line_cached_buffer =
            Rc::new(RefCell::new(CachedRenderBuffer::new(terminal.width()?, 1)));
        Ok(Monitor {
            terminal,
            theme_set,
            perference,
            region_controllers: HashMap::new(),
            render_caches: HashMap::new(),
            last_key: None,
            plugin_system,
            root_region,
            selected_region_index: 0,
            leaf_regions,
            status_line_cached_buffer,
            regions,
        })
    }

    pub fn split_region(
        &mut self,
        region: Rc<RefCell<dyn Region>>,
        split_mode: SplitMode,
        scale: usize,
        current_buffer: &Buffer,
        layout_first: bool,
    ) -> Result<Option<Rc<RefCell<dyn Region>>>> {
        let buffer_id = current_buffer.id()?;

        let (first, second) = region.borrow_mut().split(split_mode, scale, layout_first);

        self.regions.insert(first.borrow().id(), first.clone());
        self.regions.insert(second.borrow().id(), second.clone());

        self.leaf_regions.clear();
        RootRegion::fill_leaf_regions(
            &(self.root_region.clone() as Rc<RefCell<dyn Region>>),
            &mut self.leaf_regions,
        );

        let original_region = self
            .region_controllers
            .get(&buffer_id)
            .ok_or(MissingBuffer)?
            .region()
            .clone();
        if Rc::ptr_eq(&original_region, &region) {
            self.region_controllers
                .remove(&buffer_id)
                .ok_or(MissingBuffer)?;

            let mut region_controller = if layout_first {
                RegionController::new(second.clone(), 0)
            } else {
                RegionController::new(first.clone(), 0)
            };

            region_controller.scroll_into_monitor(current_buffer)?;
            let selected_region = region_controller.region().clone();
            self.region_controllers.insert(buffer_id, region_controller);
            self.update_selected_region_index(&selected_region);
        } else {
            self.update_selected_region_index(&original_region);
        }

        if layout_first {
            Ok(Some(first))
        } else {
            Ok(Some(second))
        }
    }

    fn update_selected_region_index(&mut self, selected_region: &Rc<RefCell<dyn Region>>) {
        self.selected_region_index = self
            .leaf_regions
            .iter()
            .position(|x| Rc::ptr_eq(x, selected_region))
            .unwrap();
    }

    pub fn split_root_region(
        &mut self,
        split_mode: SplitMode,
        scale: usize,
        current_buffer: &Buffer,
        layout_first: bool,
    ) -> Result<Option<Rc<RefCell<dyn Region>>>> {
        return self.split_region(
            self.root_region.clone(),
            split_mode,
            scale,
            current_buffer,
            layout_first,
        );
    }

    pub fn init_buffer(&mut self, buffer: &mut Buffer, region_id: Option<u16>) -> Result<()> {
        let region = if let Some(region_id) = region_id {
            self.leaf_regions
                .iter()
                .find(|region| region.borrow().id() == region_id)
                .clone()
                .map(|region| region.clone())
                .unwrap_or(self.leaf_regions[self.selected_region_index].clone())
        } else {
            self.root_region.clone()
        };

        let id = buffer.id()?;
        self.region_controllers
            .insert(id, RegionController::new(region, buffer.cursor.line));
        let render_cache = Rc::new(RefCell::new(HashMap::new()));
        self.render_caches.insert(id, render_cache.clone());

        // 回调清除render_cache
        buffer.change_callback = Some(Box::new(move |change_position| {
            render_cache
                .borrow_mut()
                .retain(|&k, _| k < change_position.line);
        }));

        Ok(())
    }

    pub fn bind_region(&mut self, buffer: &Buffer, region_id: u16) -> Result<()> {
        let region = self
            .leaf_regions
            .iter()
            .find(|region| region.borrow().id() == region_id)
            .clone()
            .map(|region| region.clone())
            .unwrap_or(self.leaf_regions[self.selected_region_index].clone());

        self.region_controllers.remove(&buffer.id()?);
        self.region_controllers
            .entry(buffer.id()?)
            .or_insert(RegionController::new(region, buffer.cursor.line));
        Ok(())
    }

    fn replace_region(
        &mut self,
        region: &Rc<RefCell<dyn Region>>,
        replace: Rc<RefCell<dyn Region>>,
    ) {
        if let Some(controller) = self
            .region_controllers
            .iter_mut()
            .find(|x| Rc::ptr_eq(&x.1.region(), &region))
            .map(|(_, x)| x)
        {
            controller.set_region(replace);
        }
    }

    pub fn merge_replace_children(
        &mut self,
        father: Rc<RefCell<dyn Region>>,
        first: &Rc<RefCell<dyn Region>>,
        second: &Rc<RefCell<dyn Region>>,
        drop_region: &Rc<RefCell<dyn Region>>,
    ) -> bool {
        if Rc::ptr_eq(&first, &drop_region) && first.borrow().children().is_none() {
            if let Some((sub_first, sub_second)) = second.borrow().children() {
                let id = father.borrow().id();
                sub_first.borrow_mut().set_father(id);
                sub_second.borrow_mut().set_father(id);
                father
                    .borrow_mut()
                    .set_split_data(second.borrow().split_data());
                father
                    .borrow_mut()
                    .set_children(Some((sub_first, sub_second)));
            } else {
                // 把second对应的buffer指向自身
                self.replace_region(&second, father.clone());
                self.replace_region(&first, father.clone());
                father.borrow_mut().set_children(None);
            }
            self.regions.remove(&first.borrow().id());
            self.regions.remove(&second.borrow().id());

            self.leaf_regions.clear();
            RootRegion::fill_leaf_regions(
                &(self.root_region.clone() as Rc<RefCell<dyn Region>>),
                &mut self.leaf_regions,
            );
            father.borrow_mut().clear_render_cache();

            // assert_eq!(Rc::strong_count(first), 2);
            // assert_eq!(Rc::strong_count(second), 2);

            return true;
        }

        return false;
    }

    // drop_region_id必须为叶子节点id
    pub fn merge(
        &mut self,
        region: Rc<RefCell<dyn Region>>,
        drop_region: &Rc<RefCell<dyn Region>>,
    ) -> bool {
        let children = region.borrow().children();
        if let Some((first, second)) = children {
            if Rc::ptr_eq(&region, &drop_region) {
                return false;
            }

            if self.merge_replace_children(region.clone(), &first, &second, &drop_region)
                || self.merge_replace_children(region.clone(), &second, &first, &drop_region)
            {
                return true;
            }

            return self.merge(first, drop_region) || self.merge(second, drop_region);
        }

        return false;
    }

    pub fn merge_region(&mut self, region_id: u16, drop_region: &Rc<RefCell<dyn Region>>) -> bool {
        if let Some(region) = self.regions.get(&region_id).cloned() {
            return self.merge(region, drop_region);
        }

        for (_, controller) in self.region_controllers.iter() {
            assert!(!Rc::ptr_eq(drop_region, controller.region()));
        }

        for (_, region) in self.regions.iter() {
            assert!(!Rc::ptr_eq(drop_region, &region));
        }

        for region in self.leaf_regions.iter() {
            assert!(!Rc::ptr_eq(drop_region, region));
        }
        false
    }

    pub fn deinit_buffer(&mut self, buffer: &Buffer) -> Result<()> {
        let id = buffer.id()?;
        self.region_controllers.remove(&id);
        self.render_caches.remove(&id);
        Ok(())
    }

    pub fn listen(&mut self) -> Result<Event> {
        let ev = self.terminal.listen()?;
        if let Event::Key(key) = ev {
            self.last_key.replace(key);
        }
        Ok(ev)
    }

    pub fn get_theme(&self, name: &String) -> Option<Theme> {
        self.theme_set.themes.get(name).cloned()
    }

    pub fn first_theme(&self) -> Option<Theme> {
        self.theme_set
            .themes
            .first_key_value()
            .map(|(_, v)| v.clone())
    }

    pub fn build_presenter(&mut self) -> Result<Presenter> {
        Presenter::new(
            self,
            self.leaf_regions[self.selected_region_index].clone(),
            true,
        )
    }

    pub fn update_selected_region(&mut self, buffer_id: usize) -> Result<()> {
        let region = self
            .region_controllers
            .get(&buffer_id)
            .chain_err(|| MissingBuffer)?
            .region()
            .clone();

        self.update_selected_region_index(&region);

        Ok(())
    }

    pub fn build_presenter_by_buffer(&mut self, buffer: &Buffer) -> Result<Presenter> {
        let region = self.get_region_controller(buffer).region().clone();
        let focused = Rc::ptr_eq(&self.leaf_regions[self.selected_region_index], &region);
        Presenter::new(self, region, focused)
    }

    pub fn get_render_cache(&self, buffer: &Buffer) -> &Rc<RefCell<HashMap<usize, RenderState>>> {
        self.render_caches.get(&buffer.id.unwrap()).unwrap()
    }

    pub fn get_region_controller(&mut self, buffer: &Buffer) -> &mut RegionController {
        self.region_controllers
            .get_mut(&buffer.id.unwrap())
            .expect("Trying to get a ScrollController with an uninitialized Buffer")
    }

    pub fn swap_selected_buffer_region(&mut self, buffer_id: usize) -> Result<()> {
        return self.swap_buffer_region(self.selected_region_index, buffer_id);
    }

    pub fn swap_buffer_region(&mut self, l: usize, r: usize) -> Result<()> {
        let left = self
            .region_controllers
            .remove(&l)
            .chain_err(|| "Unknown buffer")?;
        let right = self
            .region_controllers
            .remove(&r)
            .chain_err(|| "Unknown buffer")?;

        self.region_controllers.insert(l, right);
        self.region_controllers.insert(r, left);

        Ok(())
    }

    pub fn scroll_to_cursor(&mut self, buffer: &Buffer) -> Result<()> {
        let controller = self.get_region_controller(buffer);
        controller.scroll_into_monitor(buffer)
    }

    pub fn scroll_to_center(&mut self, buffer: &Buffer) -> Result<()> {
        self.get_region_controller(buffer).scroll_to_center(buffer)
    }

    pub fn scroll_up(&mut self, buffer: &Buffer, count: usize) {
        self.get_region_controller(buffer).scroll_up(count);
    }

    pub fn scroll_down(&mut self, buffer: &Buffer, count: usize) {
        self.get_region_controller(buffer).scroll_down(count);
    }

    pub fn zoom_up(&mut self, step: usize) {
        let region = self.leaf_regions[self.selected_region_index].clone();
        let father_id = region.borrow().father_id();
        if let Some(father_id) = father_id {
            if let Some(father) = self.regions.get(&father_id) {
                let (first, _) = father.borrow().children().unwrap();
                let mut split_data = father.borrow().split_data();
                if Rc::ptr_eq(&first, &region) {
                    split_data.zoom_up(step);
                } else {
                    split_data.zoom_down(step);
                }

                father.borrow_mut().set_split_data(split_data);
                father.borrow_mut().handle_resize();
            }
        }
    }

    pub fn zoom_down(&mut self, step: usize) {
        let region = self.leaf_regions[self.selected_region_index].clone();
        let father_id = region.borrow().father_id();
        if let Some(father_id) = father_id {
            if let Some(father) = self.regions.get(&father_id) {
                let (first, _) = father.borrow().children().unwrap();
                let mut split_data = father.borrow().split_data();
                if Rc::ptr_eq(&first, &region) {
                    split_data.zoom_down(step);
                } else {
                    split_data.zoom_up(step);
                }

                father.borrow_mut().set_split_data(split_data);
                father.borrow_mut().handle_resize();
            }
        }
    }

    pub fn handle_resize(&mut self) {
        self.root_region.borrow_mut().handle_resize();
    }

    pub fn present_status_line(&mut self, datas: &[StatusLineData]) -> Result<()> {
        let line_width = self.terminal.width()?;
        let line = self.terminal.height()?;

        let count = datas.len();
        let mut offset = 0;
        // 从左往右输出，最后一个参数在最后
        for (index, data) in datas.iter().enumerate() {
            let content = match count {
                1 => {
                    format!("{:width$}", data.content, width = line_width)
                }
                _ => {
                    if index == count - 1 {
                        format!(
                            "{:width$}",
                            data.content,
                            width = line_width.saturating_sub(offset)
                        )
                    } else {
                        data.content.to_owned()
                    }
                }
            };

            let len = content.len();
            self.terminal.print(
                &(line - 1, offset).into(),
                data.style,
                self.first_theme()
                    .unwrap_or_default()
                    .map_colors(data.color),
                &content,
            )?;
            offset += len;
        }

        self.terminal.present()?;
        Ok(())
    }
}
