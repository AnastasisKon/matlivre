//! The Files panel's model: a tree rooted at the opened project folder, with folders listed on
//! demand and re-listed every couple of seconds while they are open.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::icons::{self, Ic};

const RELIST: Duration = Duration::from_secs(2);

pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub dir: bool,
}

pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub icon: Ic,
    pub dir: bool,
    pub open: bool,
}

pub struct FileTree {
    /// The project folder the panel shows; `None` until one is opened.
    root: Option<PathBuf>,
    open: HashSet<PathBuf>,
    listings: HashMap<PathBuf, (Instant, Vec<Entry>)>,
}

fn hidden(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM
        meta.file_attributes() & 0x6 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

fn list_dir(dir: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Entry> = rd
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            if hidden(&meta) || name.starts_with('.') {
                return None;
            }
            Some(Entry { path: e.path(), name, dir: meta.is_dir() })
        })
        .collect();
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

impl FileTree {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root: root.filter(|p| p.is_dir()), open: HashSet::new(), listings: HashMap::new() }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Makes `dir` the project root (or closes the project with `None`).
    pub fn set_root(&mut self, dir: Option<PathBuf>) {
        self.root = dir;
        self.open.clear();
        self.listings.clear();
    }

    /// Opens every folder from the root down to `dir` (nothing if `dir` is outside the root).
    pub fn reveal(&mut self, dir: &Path) {
        let Some(root) = &self.root else { return };
        if !dir.starts_with(root) {
            return;
        }
        for a in dir.ancestors().take_while(|a| a.starts_with(root)) {
            self.open.insert(a.to_path_buf());
        }
    }

    pub fn toggle(&mut self, path: &Path) {
        if !self.open.remove(path) {
            self.open.insert(path.to_path_buf());
            self.listings.remove(path);
        }
    }

    fn children(&mut self, dir: &Path) -> &[Entry] {
        let stale = self.listings.get(dir).is_none_or(|(t, _)| t.elapsed() > RELIST);
        if stale {
            self.listings.insert(dir.to_path_buf(), (Instant::now(), list_dir(dir)));
        }
        &self.listings[dir].1
    }

    /// The visible rows below the root, depth first.
    pub fn rows(&mut self) -> Vec<Row> {
        let mut rows = Vec::new();
        if let Some(root) = self.root.clone() {
            self.walk(&root, 0, &mut rows);
        }
        rows
    }

    fn walk(&mut self, dir: &Path, depth: usize, rows: &mut Vec<Row>) {
        let entries: Vec<(PathBuf, String, bool)> = self.children(dir).iter().map(|e| (e.path.clone(), e.name.clone(), e.dir)).collect();
        for (path, name, is_dir) in entries {
            let open = is_dir && self.open.contains(&path);
            let icon = if !is_dir {
                icons::for_file(&name)
            } else if open {
                Ic::FolderOpen
            } else {
                Ic::Folder
            };
            rows.push(Row { path: path.clone(), name, depth, icon, dir: is_dir, open });
            if open {
                self.walk(&path, depth + 1, rows);
            }
        }
    }
}
