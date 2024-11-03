use std::{
    cell::Ref,
    collections::HashMap,
    env,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::{
    errors::*,
    modules::perferences::{Perferences, PerferencesManager},
    view::monitor::Monitor,
};
use syntect::parsing::SyntaxSet;

use crate::buffer::Buffer;

pub struct Workspace {
    pub path: PathBuf,
    buffers: HashMap<usize, Buffer>,
    // ino -> id
    buffers_ino_map: HashMap<u64, usize>,

    current_buffer_index: Option<usize>,

    observation_buffers: Vec<Buffer>,

    pub syntax_set: SyntaxSet,
    buffer_ida: usize,
}

impl Workspace {
    pub fn create_workspace(
        monitor: &mut Monitor,
        perferences: Ref<dyn Perferences>,
        args: &[String],
    ) -> Result<Workspace> {
        let mut path_args = args.iter().skip(1).peekable();

        let initial_dir = env::current_dir()?;
        // 若第一个参数为dir，则修改工作区
        if let Some(dir) = path_args.peek() {
            let path = Path::new(dir);
            if path.is_dir() {
                env::set_current_dir(path.canonicalize()?)?;
            }
        }
        let workspace_dir = env::current_dir()?;
        #[cfg(feature = "dragonos")]
        let syntax_path: Option<PathBuf> = None;
        #[cfg(not(feature = "dragonos"))]
        let syntax_path = PerferencesManager::user_syntax_path().map(Some)?;
        let mut workspace = Workspace::new(&workspace_dir, syntax_path.as_deref())?;

        if workspace_dir != initial_dir {
            path_args.next();
        }

        for path_str in path_args {
            let path = Path::new(path_str);
            if path.is_dir() {
                continue;
            }

            let syntax_ref = perferences
                .syntax_definition_name(&path)
                .and_then(|name| workspace.syntax_set.find_syntax_by_name(&name).cloned());

            let buffer = if path.exists() {
                let mut buffer = Buffer::from_file(&path, Some(&workspace_dir))?;
                buffer.syntax_definition = syntax_ref;
                buffer
            } else {
                let mut buffer = Buffer::new();
                buffer.syntax_definition = syntax_ref;

                if path.is_absolute() {
                    buffer.set_absolute_path(path.to_path_buf(), Some(&workspace_dir));
                } else {
                    buffer.set_absolute_path(workspace_dir.join(path), Some(&workspace_dir));
                }
                buffer
            };

            let id = workspace.add_buffer(buffer);
            workspace.select_buffer(id);
            monitor.init_buffer(workspace.current_buffer_mut().unwrap(), None)?;
        }

        Ok(workspace)
    }

    fn new(path: &Path, syntax_definitions: Option<&Path>) -> Result<Workspace> {
        let mut syntax_set = SyntaxSet::load_defaults_newlines();
        if let Some(syntax_definitions) = syntax_definitions {
            if syntax_definitions.is_dir() {
                if syntax_definitions.read_dir()?.count() > 0 {
                    let mut builder = syntax_set.into_builder();
                    builder.add_from_folder(syntax_definitions, true)?;
                    syntax_set = builder.build();
                }
            }
        }

        Ok(Workspace {
            path: path.canonicalize()?,
            buffers: HashMap::new(),
            buffers_ino_map: HashMap::new(),
            syntax_set,
            buffer_ida: 0,
            current_buffer_index: None,
            observation_buffers: Default::default(),
        })
    }

    pub fn add_buffer(&mut self, mut buffer: Buffer) -> usize {
        let id = self.alloc_buffer_id();
        buffer.id = Some(id);

        if let Some(ref path) = buffer.absolute_path() {
            if let Ok(metadata) = path.metadata() {
                self.buffers_ino_map.insert(metadata.ino(), id);
            }
        }
        self.update_current_syntax(&mut buffer);
        self.buffers.insert(id, buffer);

        return id;
    }

    fn alloc_buffer_id(&mut self) -> usize {
        self.buffer_ida += 1;
        self.buffer_ida
    }

    pub fn get_buffer(&self, id: usize) -> Option<&Buffer> {
        if let Some(buffer) = self.current_buffer() {
            if buffer.id.unwrap() == id {
                return Some(buffer);
            }
        }

        if let Some(buffer) = self.observation_buffers.iter().find(|x| x.id == Some(id)) {
            return Some(buffer);
        }

        return self.buffers.get(&id);
    }

    pub fn get_buffer_mut(&mut self, id: usize) -> Option<&mut Buffer> {
        if self.current_buffer_mut().is_some() && self.current_buffer_mut().unwrap().id == Some(id)
        {
            return self.current_buffer_mut();
        }

        if let Some(buffer) = self
            .observation_buffers
            .iter_mut()
            .find(|x| x.id == Some(id))
        {
            return Some(buffer);
        }

        return self.buffers.get_mut(&id);
    }

    pub fn get_buffer_with_ino(&self, ino: u64) -> Option<&Buffer> {
        if let Some(id) = self.buffers_ino_map.get(&ino) {
            return self.get_buffer(*id);
        }
        None
    }

    pub fn select_buffer(&mut self, id: usize) -> bool {
        if let Some(index) = self.current_buffer_index {
            if self.observation_buffers[index].id == Some(id) {
                return true;
            }
        }

        if let Some(index) = self
            .observation_buffers
            .iter()
            .position(|x| x.id == Some(id))
        {
            let buffer = self.observation_buffers.remove(index);
            self.buffers.insert(id, buffer);
        }

        // 选择新buffer
        if let Some(buffer) = self.buffers.remove(&id) {
            if let Some(current_index) = self.current_buffer_index {
                self.cancel_observe_buffer(self.observation_buffers[current_index].id.unwrap());
            }
            self.observation_buffers.push(buffer);
            self.current_buffer_index = Some(self.observation_buffers.len() - 1);
            return true;
        }

        false
    }

    pub fn change_to_target_observation_buffer(
        &mut self,
        id: usize,
        monitor: &mut Monitor,
    ) -> Result<bool> {
        if let Some(index) = self
            .observation_buffers
            .iter()
            .position(|x| x.id == Some(id))
        {
            self.current_buffer_index = Some(index);
            monitor.update_selected_region(id)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn change_to_next_observation_buffer(&mut self, monitor: &mut Monitor) -> Result<()> {
        loop {
            self.current_buffer_index = self
                .current_buffer_index
                .map(|index| (index + 1) % self.observation_buffers.len());

            if self.observation_buffers[self.current_buffer_index.unwrap()]
                .absolute_path()
                .is_some()
            {
                monitor.update_selected_region(
                    self.observation_buffers[self.current_buffer_index.unwrap()].id()?,
                )?;
                break;
            }
        }

        Ok(())
    }

    pub fn update_current_syntax(&mut self, buffer: &mut Buffer) {
        let syntax_reference = self.syntax_set.find_syntax_plain_text();
        let definition = buffer
            .file_extension()
            .and_then(|ex| self.syntax_set.find_syntax_by_extension(&ex))
            .or_else(|| Some(syntax_reference))
            .cloned();
        buffer.syntax_definition = definition;
    }

    pub fn observe_buffer(&mut self, buffer_id: usize) {
        if let Some(buffer) = self.buffers.remove(&buffer_id) {
            self.observation_buffers.push(buffer);
        }
    }

    pub fn cancel_observe_buffer(&mut self, buffer_id: usize) {
        if let Some(buffer_index) = self
            .observation_buffers
            .iter()
            .position(|x| x.id == Some(buffer_id))
        {
            let buffer = self.observation_buffers.remove(buffer_index);
            self.buffers.insert(buffer_id, buffer);
        }

        if Some(buffer_id) == self.current_buffer_index {
            if self.observation_buffers.is_empty() {
                self.current_buffer_index = None;
            } else {
                self.current_buffer_index = self
                    .current_buffer_index
                    .map(|x| x % self.observation_buffers.len());
            }
        }
    }

    pub fn get_observation_buffers_iter(&self) -> impl Iterator<Item = &Buffer> {
        self.observation_buffers.iter()
    }

    pub fn current_buffer(&self) -> Option<&Buffer> {
        if let Some(index) = self.current_buffer_index {
            self.observation_buffers.get(index)
        } else {
            None
        }
    }

    pub fn current_buffer_mut(&mut self) -> Option<&mut Buffer> {
        if let Some(index) = self.current_buffer_index {
            self.observation_buffers.get_mut(index)
        } else {
            None
        }
    }
}
