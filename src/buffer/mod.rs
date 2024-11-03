use crate::errors::*;
use std::cell::RefCell;
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};

use cursor::Cursor;
use held_core::utils::position::Position;
use operation::history::History;
use operation::{Operation, OperationGroup};
use syntect::parsing::SyntaxReference;

use crate::errors::Error;
use held_core::utils::range::Range;

// Published API
pub use self::gap_buffer::GapBuffer;

mod cursor;
mod gap_buffer;
mod operation;

pub struct Buffer {
    pub id: Option<usize>,
    data: Rc<RefCell<GapBuffer>>,
    path: Option<PathBuf>,
    relative_path: Option<PathBuf>,
    pub cursor: Cursor,
    history: History,
    operation_group: Option<OperationGroup>,
    pub syntax_definition: Option<SyntaxReference>,
    pub change_callback: Option<Box<dyn Fn(Position)>>,
}

impl Default for Buffer {
    fn default() -> Self {
        let data = Rc::new(RefCell::new(GapBuffer::new(String::new())));
        let cursor = Cursor::new(data.clone(), Position { line: 0, offset: 0 });
        let mut history = History::new();
        history.mark();

        Buffer {
            id: None,
            data: data.clone(),
            path: None,
            cursor,
            history: History::new(),
            operation_group: None,
            syntax_definition: None,
            change_callback: None,
            relative_path: None,
        }
    }
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer::default()
    }

    pub fn from_file(path: &Path, workspace_dir: Option<&PathBuf>) -> io::Result<Buffer> {
        let content = fs::read_to_string(path)?;

        let data = Rc::new(RefCell::new(GapBuffer::new(content)));
        let cursor = Cursor::new(data.clone(), Position { line: 0, offset: 0 });

        let relative_path = if let Some(workspace_dir) = workspace_dir {
            if let Ok(relative) = path.strip_prefix(workspace_dir) {
                Some(relative.to_path_buf())
            } else {
                Some(path.to_path_buf())
            }
        } else {
            Some(path.to_path_buf())
        };

        let mut buffer = Buffer {
            id: None,
            data: data.clone(),
            path: Some(path.canonicalize()?),
            cursor,
            history: History::new(),
            operation_group: None,
            syntax_definition: None,
            change_callback: None,
            relative_path,
        };

        buffer.history.mark();

        Ok(buffer)
    }

    pub fn data(&self) -> String {
        self.data.borrow().to_string()
    }

    pub fn save(&mut self) -> io::Result<()> {
        let mut file = if let Some(ref path) = self.path {
            File::create(path)?
        } else {
            File::create(PathBuf::new())?
        };

        file.write_all(self.data().to_string().as_bytes())?;

        self.history.mark();

        Ok(())
    }

    pub fn file_name(&self) -> Option<String> {
        self.path.as_ref().and_then(|p| {
            p.file_name()
                .and_then(|f| f.to_str().map(|s| s.to_string()))
        })
    }

    pub fn undo(&mut self) {
        // Look for an operation to undo. First, check if there's an open, non-empty
        // operation group. If not, try taking the last operation from the buffer history.
        let operation: Option<Box<dyn Operation>> = match self.operation_group.take() {
            Some(group) => {
                if group.is_empty() {
                    self.history.previous()
                } else {
                    Some(Box::new(group))
                }
            }
            None => self.history.previous(),
        };

        // If we found an eligible operation, reverse it.
        if let Some(mut op) = operation {
            op.reverse(self);
        }
    }

    pub fn redo(&mut self) {
        // Look for an operation to apply.
        if let Some(mut op) = self.history.next() {
            op.run(self);
        }
    }

    pub fn read(&self, range: &Range) -> Option<String> {
        self.data.borrow().read(range)
    }

    pub fn search(&self, needle: &str) -> Vec<Position> {
        let mut results = Vec::new();

        for (line, data) in self.data().lines().enumerate() {
            for (offset, _) in data.char_indices() {
                let haystack = &data[offset..];

                // Check haystack length before slicing it and comparing bytes with needle.
                if haystack.len() >= needle.len()
                    && needle.as_bytes() == &haystack.as_bytes()[..needle.len()]
                {
                    results.push(Position { line, offset });
                }
            }
        }

        results
    }

    pub fn modified(&self) -> bool {
        !self.history.at_mark()
    }

    pub fn line_count(&self) -> usize {
        self.data().chars().filter(|&c| c == '\n').count() + 1
    }

    pub fn reload(&mut self) -> io::Result<()> {
        // Load content from disk.
        let path = self.path.as_ref().ok_or(ErrorKind::NotFound)?;
        let content = fs::read_to_string(path)?;

        self.replace(content);

        // We mark the history at points where the
        // buffer is in sync with its file equivalent.
        self.history.mark();

        Ok(())
    }

    /// 文件拓展名
    pub fn file_extension(&self) -> Option<String> {
        self.path.as_ref().and_then(|p| {
            p.extension().and_then(|e| {
                if !e.is_empty() {
                    return Some(e.to_string_lossy().into_owned());
                }

                None
            })
        })
    }

    pub fn id(&self) -> Result<usize> {
        self.id
            .ok_or_else(|| Error::from("Buffer ID doesn't exist"))
    }

    pub fn set_absolute_path(&mut self, path: PathBuf, workspace_dir: Option<&PathBuf>) {
        let relative_path = if let Some(workspace_dir) = workspace_dir {
            if let Ok(relative) = path.strip_prefix(workspace_dir) {
                relative.to_path_buf()
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };
        self.path = Some(path);
        self.relative_path = Some(relative_path);
    }

    pub fn absolute_path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    pub fn relative_path(&self) -> Option<PathBuf> {
        self.relative_path.clone()
    }
}
