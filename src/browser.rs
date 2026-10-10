//! State for the built-in file browser (directory listing, filtering, selection).

use crate::util;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrowseMode {
    /// Pick media to play.
    Media,
    /// Pick an external subtitle for the current file.
    Subtitle,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

pub struct Browser {
    pub mode: BrowseMode,
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub filter: String,
    pub selected: usize,
    pub scroll: f32,
    pub show_hidden: bool,
    pub show_all: bool,
    pub error: Option<String>,
}

impl Browser {
    pub fn new(mode: BrowseMode, start: &Path) -> Self {
        let mut b = Self {
            mode,
            cwd: PathBuf::new(),
            entries: Vec::new(),
            filter: String::new(),
            selected: 0,
            scroll: 0.0,
            show_hidden: false,
            show_all: false,
            error: None,
        };
        let start = if start.is_dir() { start.to_path_buf() } else { util::home_dir() };
        b.navigate(&start);
        b
    }

    fn wanted(&self, name: &str) -> bool {
        if self.show_all {
            return true;
        }
        match self.mode {
            BrowseMode::Media => util::is_media(name),
            BrowseMode::Subtitle => util::is_subtitle(name),
        }
    }

    pub fn navigate(&mut self, dir: &Path) {
        let prev = self.cwd.clone();
        match std::fs::read_dir(dir) {
            Ok(rd) => {
                self.cwd = dir.to_path_buf();
                self.error = None;
                let mut entries: Vec<Entry> = rd
                    .flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        if !self.show_hidden && name.starts_with('.') {
                            return None;
                        }
                        let path = e.path();
                        // Follow symlinks so linked folders behave like folders.
                        let meta = std::fs::metadata(&path).ok()?;
                        let is_dir = meta.is_dir();
                        if !is_dir && !self.wanted(&name) {
                            return None;
                        }
                        Some(Entry { name, path, is_dir, size: if is_dir { 0 } else { meta.len() } })
                    })
                    .collect();
                entries.sort_by(|a, b| {
                    b.is_dir.cmp(&a.is_dir).then_with(|| natural_cmp(&a.name.to_lowercase(), &b.name.to_lowercase()))
                });
                self.entries = entries;
                self.filter.clear();
                self.scroll = 0.0;
                // When going up, keep the folder we came from selected.
                self.selected = self.entries.iter().position(|e| e.path == prev).unwrap_or(0);
            }
            Err(e) => self.error = Some(format!("{}: {e}", dir.display())),
        }
    }

    pub fn refresh(&mut self) {
        let sel = self.visible().get(self.selected).map(|e| e.path.clone());
        let cwd = self.cwd.clone();
        self.navigate(&cwd);
        if let Some(p) = sel {
            self.selected = self.visible().iter().position(|e| e.path == p).unwrap_or(0);
        }
    }

    pub fn up(&mut self) {
        if let Some(parent) = self.cwd.parent().map(Path::to_path_buf) {
            let from = self.cwd.clone();
            self.navigate(&parent);
            self.selected = self.visible().iter().position(|e| e.path == from).unwrap_or(0);
        }
    }

    pub fn visible(&self) -> Vec<&Entry> {
        if self.filter.is_empty() {
            return self.entries.iter().collect();
        }
        let f = self.filter.to_lowercase();
        self.entries.iter().filter(|e| e.name.to_lowercase().contains(&f)).collect()
    }

    pub fn set_filter(&mut self, f: String) {
        self.filter = f;
        self.selected = 0;
        self.scroll = 0.0;
    }

    pub fn move_sel(&mut self, delta: isize) {
        let n = self.visible().len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as isize + delta).clamp(0, n as isize - 1) as usize;
    }

    pub fn current(&self) -> Option<Entry> {
        self.visible().get(self.selected).map(|e| (*e).clone())
    }

    /// All playable files in the current directory, in display order.
    pub fn files_here(&self) -> Vec<String> {
        self.visible().iter().filter(|e| !e.is_dir).map(|e| e.path.to_string_lossy().into_owned()).collect()
    }
}

/// Compare strings treating digit runs numerically ("ep2" < "ep10").
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = ai.peek().copied().filter(char::is_ascii_digit) {
                    na.push(c);
                    ai.next();
                }
                let mut nb = String::new();
                while let Some(c) = bi.peek().copied().filter(char::is_ascii_digit) {
                    nb.push(c);
                    bi.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                ai.next();
                bi.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;
    use std::cmp::Ordering;

    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("ep2", "ep10"), Ordering::Less);
        assert_eq!(natural_cmp("ep010", "ep10"), Ordering::Equal);
        assert_eq!(natural_cmp("a", "b"), Ordering::Less);
        assert_eq!(natural_cmp("ep10x", "ep10"), Ordering::Greater);
    }
}
