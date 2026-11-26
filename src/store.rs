//! The workspace: a plain folder of Markdown files. Any sync tool
//! (Google Drive, OneDrive, Dropbox, iCloud, Syncthing...) can share it.

use crate::model::{self, Block, Kind, Page};
use std::collections::HashMap;
use crate::vfs as fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub fn now_secs() -> u64 {
    fs::now_millis() / 1000
}

pub fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0x9E37_79B9);
    let n = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let c = COUNTER.fetch_add(0x6C8E_9CF5, Ordering::Relaxed);
    let mut x = (n as u64) ^ ((n >> 64) as u64) ^ c.rotate_left(29);
    // xorshift for spread
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    format!("{:08x}", x as u32)
}

fn sanitize(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| if "/\\:*?\"<>|\n\r\t".contains(c) { ' ' } else { c })
        .collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s: String = s.chars().take(60).collect();
    let s = s.trim_matches('.').trim().to_string();
    if s.is_empty() { "Untitled".into() } else { s }
}

pub struct Entry {
    pub page: Page,
    pub path: PathBuf,
    /// Last change of the file, in milliseconds.
    pub mtime: u64,
    pub dirty: bool,
    pub last_edit: f64,
}

pub struct Workspace {
    pub root: PathBuf,
    /// Counts edits since the workspace was opened (for "ask before closing").
    pub edits: u64,
    pub entries: Vec<Entry>,
    index: HashMap<String, usize>,
    pub last_error: Option<String>,
}

impl Workspace {
    pub fn open(root: PathBuf) -> Self {
        let _ = fs::create_dir_all(&root);
        let mut ws = Workspace { root, edits: 0, entries: Vec::new(), index: HashMap::new(), last_error: None };
        ws.full_scan();
        ws
    }

    pub fn name(&self) -> String {
        self.root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Workspace".into())
    }

    fn reindex(&mut self) {
        self.index.clear();
        for (i, e) in self.entries.iter().enumerate() {
            self.index.insert(e.page.id.clone(), i);
        }
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.index.get(id).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Entry> {
        match self.index.get(id) {
            Some(&i) => Some(&mut self.entries[i]),
            None => None,
        }
    }

    /// Every page file, including ones inside page folders. Hidden files and
    /// the `_assets` folder are skipped.
    fn md_files(&self) -> Vec<(PathBuf, u64)> {
        fs::list_md(&self.root)
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("_assets")
    }

    /// Copy a picture into the workspace so it syncs with the notes.
    /// Returns its path relative to the workspace.
    pub fn import_asset(&self, file_name: &str, bytes: &[u8]) -> Result<String, String> {
        let dir = self.assets_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let fp = Path::new(file_name);
        let stem = sanitize(&fp.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
        let stem = if stem == "Untitled" { "picture".to_string() } else { stem };
        let ext = fp.extension().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_else(|| "png".into());
        let mut name = format!("{stem}.{ext}");
        let mut n = 2;
        while fs::exists(&dir.join(&name)) {
            // Same file already imported? Reuse it.
            if fs::len(&dir.join(&name)) == Some(bytes.len() as u64) {
                return Ok(format!("_assets/{name}"));
            }
            name = format!("{stem} {n}.{ext}");
            n += 1;
        }
        fs::write_atomic(&dir.join(&name), bytes).map_err(|e| e.to_string())?;
        Ok(format!("_assets/{name}"))
    }

    /// Bring files into the notes folder: `.md` pages, pictures, or a `.zip`
    /// of a whole notes folder. Returns how many files were added.
    pub fn import_files(&mut self, files: Vec<(String, Vec<u8>)>) -> Result<usize, String> {
        let mut items: Vec<(String, Vec<u8>)> = Vec::new();
        for (name, bytes) in files {
            if name.to_lowercase().ends_with(".zip") {
                let mut inner = crate::zip::read(&bytes)?;
                inner.retain(|(n, _)| !n.starts_with("__MACOSX/") && !n.split('/').any(|p| p.starts_with('.')));
                // Drop a shared top folder ("My Notes/...") so pages land at the top level.
                let first: Option<String> = inner.first().and_then(|(n, _)| n.split('/').next().map(|s| s.to_string()));
                if let Some(top) = first {
                    if inner.iter().all(|(n, _)| n.starts_with(&format!("{top}/"))) {
                        for (n, _) in &mut inner {
                            *n = n[top.len() + 1..].to_string();
                        }
                    }
                }
                items.extend(inner);
            } else {
                let lower = name.to_lowercase();
                if crate::platform::IMAGE_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}"))) {
                    items.push((format!("_assets/{name}"), bytes));
                } else {
                    items.push((name, bytes));
                }
            }
        }
        let mut n = 0;
        for (rel, bytes) in items {
            let rel = rel.trim_start_matches('/').replace('\\', "/");
            if rel.is_empty() || rel.split('/').any(|p| p == "..") {
                continue;
            }
            let mut dest = self.root.join(&rel);
            if rel.ends_with(".md") {
                // Never overwrite a different page with the same name.
                let mut k = 2;
                while fs::exists(&dest) && fs::read(&dest).ok().as_deref() != Some(&bytes[..]) {
                    let stem = Path::new(&rel).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let parent = Path::new(&rel).parent().map(|p| p.to_path_buf()).unwrap_or_default();
                    dest = self.root.join(parent).join(format!("{stem} {k}.md"));
                    k += 1;
                }
            }
            fs::write_atomic(&dest, &bytes).map_err(|e| e.to_string())?;
            n += 1;
        }
        self.poll_external();
        Ok(n)
    }

    /// The whole notes folder as a .zip (pages, folders and pictures).
    pub fn export_zip(&mut self) -> Vec<u8> {
        let top = self.name();
        self.export_zip_named(&top)
    }

    /// The notes as a .zip whose top folder is called `top`.
    pub fn export_zip_named(&mut self, top: &str) -> Vec<u8> {
        self.flush(0.0, true);
        let top = sanitize(top);
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for p in fs::list_all(&self.root) {
            if let (Ok(rel), Ok(bytes)) = (p.strip_prefix(&self.root), fs::read(&p)) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                if rel.ends_with(".tmp") {
                    continue;
                }
                files.push((format!("{top}/{rel}"), bytes));
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        crate::zip::write(&files)
    }

    pub fn resolve(&self, rel: &str) -> PathBuf {
        let p = Path::new(rel);
        if p.is_absolute() { p.to_path_buf() } else { self.root.join(rel) }
    }

    /// Turn "keep sub-pages in a folder" on or off; files move on the next save.
    pub fn set_folder(&mut self, id: &str, on: bool, now: f64) {
        if let Some(e) = self.get_mut(id) {
            e.page.folder = on;
        }
        self.touch(id, now - 10.0);
        for d in self.descendants(id) {
            self.touch(&d, now - 10.0);
        }
    }

    fn load_file(path: &Path) -> Option<Page> {
        let s = fs::read_to_string(path).ok()?;
        let mut p = model::from_markdown(&s);
        if p.title.is_empty() {
            p.title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        }
        Some(p)
    }

    fn full_scan(&mut self) {
        self.entries.clear();
        let mut files = self.md_files();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        for (path, mtime) in files {
            if let Some(mut page) = Self::load_file(&path) {
                let mut needs_save = false;
                if page.id.is_empty() || self.entries.iter().any(|e| e.page.id == page.id) {
                    // A plain Markdown file dropped in, or a sync-conflict copy.
                    page.id = new_id();
                    needs_save = true;
                }
                self.entries.push(Entry { page, path, mtime, dirty: needs_save, last_edit: 0.0 });
            }
        }
        self.reindex();
        self.infer_folder_parents();
    }

    /// A page file dropped inside "X/" next to "X.md" belongs to page X.
    fn infer_folder_parents(&mut self) {
        let mut links: Vec<(usize, String, usize)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            if !e.page.parent.is_empty() {
                continue;
            }
            let Some(dir) = e.path.parent() else { continue };
            if dir == self.root {
                continue;
            }
            let owner = dir.with_extension("md");
            if let Some(j) = self.entries.iter().position(|o| o.path == owner) {
                links.push((i, self.entries[j].page.id.clone(), j));
            }
        }
        for (i, pid, j) in links {
            self.entries[i].page.parent = pid;
            self.entries[i].dirty = true;
            self.entries[j].page.folder = true;
        }
    }

    /// Pick up changes made by a sync client or another device.
    /// Returns true when something changed.
    pub fn poll_external(&mut self) -> bool {
        let files = self.md_files();
        let mut changed = false;
        let mut seen: Vec<PathBuf> = Vec::new();
        for (path, mtime) in files {
            seen.push(path.clone());
            let pos = self.entries.iter().position(|e| e.path == path);
            match pos {
                Some(i) => {
                    let e = &mut self.entries[i];
                    if e.mtime != mtime && !e.dirty {
                        if let Some(mut page) = Self::load_file(&path) {
                            if page.id.is_empty() {
                                page.id = e.page.id.clone();
                            }
                            e.page = page;
                            e.mtime = mtime;
                            changed = true;
                        }
                    }
                }
                None => {
                    if let Some(mut page) = Self::load_file(&path) {
                        let mut dirty = false;
                        if page.id.is_empty() || self.index.contains_key(&page.id) {
                            page.id = new_id();
                            dirty = true;
                        }
                        self.entries.push(Entry { page, path, mtime, dirty, last_edit: 0.0 });
                        changed = true;
                    }
                }
            }
        }
        let before = self.entries.len();
        self.entries.retain(|e| e.dirty || seen.contains(&e.path));
        if self.entries.len() != before {
            changed = true;
        }
        if changed {
            self.reindex();
        }
        changed
    }

    /// Folder where the children of `id` are stored.
    fn children_dir(&self, id: &str, depth: usize) -> PathBuf {
        if id.is_empty() || depth > 32 {
            return self.root.clone();
        }
        let Some(i) = self.index.get(id).copied() else { return self.root.clone() };
        let e = &self.entries[i];
        if e.page.folder {
            let own = if e.path.as_os_str().is_empty() { self.path_for_idx(i, depth + 1) } else { e.path.clone() };
            own.with_extension("")
        } else {
            self.children_dir(&e.page.parent, depth + 1)
        }
    }

    /// Where page `idx` should live: "<folder>/<Title>.md", with " 2", " 3"... on clashes.
    /// A folder that belongs to a page ("Keep sub-pages in a folder").
    fn is_page_folder(&self, d: &Path) -> bool {
        let owner = d.with_extension("md");
        self.entries.iter().any(|o| o.path == owner || (o.page.folder && !o.path.as_os_str().is_empty() && o.path.with_extension("") == d))
    }

    fn path_for_idx(&self, idx: usize, depth: usize) -> PathBuf {
        let e = &self.entries[idx];
        let mut dir = self.children_dir(&e.page.parent, depth + 1);
        // A page in a folder the person made themselves stays where it is
        // (unless the app means to put it into a page's folder).
        if let Some(cur) = e.path.parent() {
            if !e.path.as_os_str().is_empty()
                && cur != dir
                && cur != self.root
                && cur.starts_with(&self.root)
                && !self.is_page_folder(cur)
                && !self.is_page_folder(&dir)
            {
                dir = cur.to_path_buf();
            }
        }
        let base = sanitize(e.page.display_title());
        // Keep the current name if it still matches the title.
        if e.path.parent() == Some(dir.as_path()) {
            if let Some(stem) = e.path.file_stem().map(|s| s.to_string_lossy().to_string()) {
                let rest = stem.strip_prefix(&base).unwrap_or("x");
                let rest = rest.trim();
                if rest.is_empty() || rest.chars().all(|c| c.is_ascii_alphanumeric()) && rest.len() <= 4 && stem.starts_with(&base) && stem.len() > base.len() && stem.as_bytes()[base.len()] == b' ' {
                    return e.path.clone();
                }
            }
        }
        let mut n = 1;
        loop {
            let name = if n == 1 { format!("{base}.md") } else { format!("{base} {n}.md") };
            let cand = dir.join(&name);
            let taken = self.entries.iter().enumerate().any(|(j, o)| j != idx && o.path == cand) || (cand.exists() && cand != e.path);
            if !taken {
                return cand;
            }
            n += 1;
        }
    }

    pub fn save_entry(&mut self, idx: usize) {
        let want = self.path_for_idx(idx, 0);
        if let Some(dir) = want.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let old = self.entries[idx].path.clone();
        // A folder page that moves or is renamed takes its folder along.
        if self.entries[idx].page.folder && !old.as_os_str().is_empty() && old != want {
            let old_dir = old.with_extension("");
            let new_dir = want.with_extension("");
            if fs::is_dir(&old_dir) && !fs::exists(&new_dir) && fs::rename(&old_dir, &new_dir).is_ok() {
                for o in &mut self.entries {
                    if let Ok(rest) = o.path.strip_prefix(&old_dir) {
                        o.path = new_dir.join(rest);
                    }
                }
            }
        }
        let e = &mut self.entries[idx];
        let text = model::to_markdown(&e.page);
        let res = fs::write_atomic(&want, text.as_bytes());
        match res {
            Ok(()) => {
                if e.path != want && !e.path.as_os_str().is_empty() && fs::exists(&e.path) {
                    let _ = fs::remove_file(&e.path);
                    let old_parent = e.path.parent().map(|p| p.to_path_buf());
                    e.path = want;
                    if let Some(d) = old_parent {
                        self.remove_empty_dirs(&d);
                    }
                } else {
                    e.path = want;
                }
                let e = &mut self.entries[idx];
                e.mtime = fs::mtime(&e.path);
                e.dirty = false;
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(format!("Could not save: {err}"));
            }
        }
    }

    fn remove_empty_dirs(&self, start: &Path) {
        let mut d = start.to_path_buf();
        while d != self.root && d.starts_with(&self.root) {
            if !fs::remove_dir_if_empty(&d) {
                break;
            }
            match d.parent() {
                Some(p) => d = p.to_path_buf(),
                None => break,
            }
        }
    }

    /// Save pages that have been idle for a moment (or everything when `force`).
    pub fn flush(&mut self, now: f64, force: bool) -> bool {
        let mut any = false;
        for i in 0..self.entries.len() {
            let e = &self.entries[i];
            // Phones close apps abruptly, so the web version saves sooner.
            let idle = if cfg!(target_arch = "wasm32") { 0.25 } else { 0.6 };
            if e.dirty && (force || now - e.last_edit > idle) {
                self.save_entry(i);
                any = true;
            }
        }
        any
    }

    pub fn has_unsaved(&self) -> bool {
        self.entries.iter().any(|e| e.dirty)
    }

    pub fn touch(&mut self, id: &str, now: f64) {
        self.edits += 1;
        if let Some(e) = self.get_mut(id) {
            e.dirty = true;
            e.last_edit = now;
            e.page.updated = now_secs();
        }
    }

    pub fn create(&mut self, title: &str, parent: &str, now: f64) -> String {
        let id = loop {
            let id = new_id();
            if !self.index.contains_key(&id) {
                break id;
            }
        };
        let mut page = Page::new(id.clone(), title, parent);
        page.order = self
            .entries
            .iter()
            .filter(|e| e.page.parent == parent)
            .map(|e| e.page.order + 1)
            .max()
            .unwrap_or(0);
        self.entries.push(Entry { page, path: PathBuf::new(), mtime: 0, dirty: true, last_edit: now - 10.0 });
        self.reindex();
        id
    }

    pub fn insert_page(&mut self, page: Page) {
        let id = page.id.clone();
        self.entries.push(Entry { page, path: PathBuf::new(), mtime: 0, dirty: true, last_edit: 0.0 });
        self.reindex();
        let _ = id;
    }

    pub fn children(&self, parent: &str) -> Vec<&Page> {
        let mut v: Vec<&Page> = self
            .entries
            .iter()
            .map(|e| &e.page)
            .filter(|p| !p.trashed && p.parent == parent)
            .collect();
        // Pages whose parent is gone become top level.
        if parent.is_empty() {
            for e in &self.entries {
                let p = &e.page;
                if !p.trashed && !p.parent.is_empty() && !self.index.contains_key(&p.parent) {
                    v.push(p);
                }
            }
        }
        v.sort_by(|a, b| a.order.cmp(&b.order).then(a.created.cmp(&b.created)));
        v
    }

    pub fn descendants(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![id.to_string()];
        while let Some(cur) = stack.pop() {
            for e in &self.entries {
                if e.page.parent == cur && !out.contains(&e.page.id) {
                    out.push(e.page.id.clone());
                    stack.push(e.page.id.clone());
                }
            }
        }
        out
    }

    pub fn set_trashed(&mut self, id: &str, trashed: bool, now: f64) {
        let mut ids = self.descendants(id);
        ids.push(id.to_string());
        for i in ids {
            if let Some(e) = self.get_mut(&i) {
                e.page.trashed = trashed;
            }
            self.touch(&i, now - 10.0);
        }
    }

    pub fn delete_forever(&mut self, id: &str) {
        let mut ids = self.descendants(id);
        ids.push(id.to_string());
        let mut dirs = Vec::new();
        for i in &ids {
            if let Some(e) = self.get(i) {
                if fs::exists(&e.path) {
                    let _ = fs::remove_file(&e.path);
                    if let Some(d) = e.path.parent() {
                        dirs.push(d.to_path_buf());
                    }
                }
            }
        }
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for d in dirs {
            self.remove_empty_dirs(&d);
        }
        self.entries.retain(|e| !ids.contains(&e.page.id));
        self.reindex();
    }

    pub fn path_to(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = id.to_string();
        let mut guard = 0;
        while let Some(e) = self.get(&cur) {
            out.push(cur.clone());
            if e.page.parent.is_empty() || guard > 32 {
                break;
            }
            cur = e.page.parent.clone();
            guard += 1;
        }
        out.reverse();
        out
    }

    pub fn seed_welcome(&mut self) {
        let now = 0.0;
        let todo = self.create("To-do", "", now);
        {
            let e = self.get_mut(&todo).unwrap();
            e.page.icon = "☑".into();
            e.page.cover = 5;
            e.page.favorite = true;
            e.page.blocks.clear();
            e.page.db = Some(model::Db {
                columns: vec![
                    model::Column { name: "Done".into(), kind: model::ColKind::Checkbox, options: vec![] },
                    model::Column { name: "Due".into(), kind: model::ColKind::Date, options: vec![] },
                    model::Column { name: "Priority".into(), kind: model::ColKind::Select, options: vec!["High".into(), "Medium".into(), "Low".into()] },
                    model::Column { name: "Today".into(), kind: model::ColKind::Checkbox, options: vec![] },
                    model::Column { name: "Tags".into(), kind: model::ColKind::Multi, options: vec!["Home".into(), "Work".into(), "Errand".into()] },
                ],
                view: model::View::Table,
                group: "Priority".into(),
                filters: vec![],
                sorts: vec![],
            });
        }
        let tasks: [(&str, &str, &str, bool, &str); 8] = [
            ("Book a dentist appointment", "", "High", true, "Errand"),
            ("Renew passport", "2026-12-10", "High", false, "Errand"),
            ("Plan team offsite", "2026-12-02", "Medium", false, "Work"),
            ("Read chapter 3", "", "Low", true, "Home"),
            ("Fix bike brakes", "", "Medium", false, "Home"),
            ("Send invoice for October", "2026-11-28", "High", false, "Work"),
            ("Call the bank about the card", "", "", false, "Errand"),
            ("Try the new pasta recipe", "", "Low", false, "Home"),
        ];
        for (k, (t, due, pri, today, tag)) in tasks.iter().enumerate() {
            let rid = self.create(t, &todo, now);
            let e = self.get_mut(&rid).unwrap();
            e.page.order = k as i64;
            e.page.set_prop("Due", due.to_string());
            e.page.set_prop("Priority", pri.to_string());
            e.page.set_prop("Today", if *today { "true".into() } else { String::new() });
            e.page.set_prop("Tags", tag.to_string());
            if k == 3 {
                e.page.set_prop("Done", "true".into());
            }
        }
        let id = self.create("Getting Started", "", now);
        {
            let e = self.get_mut(&id).unwrap();
            e.page.icon = "👋".into();
            e.page.cover = 1;
            let mut b = vec![
                Block::new(
                    Kind::Callout,
                    if crate::platform::IS_WEB {
                        "Everything here is saved on this device and works offline. Use Import and Export in the menu at the bottom left to move your notes to and from the Files app, Google Drive or OneDrive."
                    } else {
                        "Everything here lives in plain Markdown files on your computer. Put the folder inside Google Drive, OneDrive or Dropbox to share it."
                    },
                ),
                Block::new(Kind::H1, "The basics"),
                Block::new(Kind::Todo, "Click anywhere and just start typing"),
                Block::new(Kind::Todo, "Type / to add headings, lists, to-dos, quotes, code and tables"),
                Block::new(Kind::Todo, "Use **bold**, *italic*, `code` and ~~strikethrough~~ inline"),
                Block::new(Kind::Todo, "Press Tab to indent list items, Shift+Tab to outdent"),
                Block::new(Kind::Todo, "Hover a block and use ⋮⋮ to move, duplicate or turn it into something else"),
                Block::new(Kind::H2, "Shortcuts"),
                Block::new(Kind::Bullet, "⌘K or ⌘P — search every page"),
                Block::new(Kind::Bullet, "⌘N — new page"),
                Block::new(Kind::Bullet, "Type # , - , [] , 1. or > at the start of a line"),
                Block::new(Kind::Divider, ""),
                Block::new(Kind::Toggle, "Where are my files?"),
                Block::new(Kind::Text, ""),
            ];
            b[2].checked = true;
            b[12].extra = "Use the workspace menu at the bottom left and choose \"Show folder\".\nEach page is one .md file you can open in any editor.".into();
            b.pop();
            b.push(Block::new(Kind::H2, "Links and pictures"));
            b.push(Block::new(Kind::Text, "Paste a link to choose Mention, URL or Preview — like [@example.com](https://example.com) here."));
            let mut bm = Block::new(Kind::Bookmark, "Example Domain");
            bm.extra = "https://example.com/".into();
            bm.desc = "A preview card. Edit the title and note to remember why you saved it.".into();
            b.push(bm);
            b.push(Block::new(Kind::Text, "Drop a picture on the page or type /image. Drag the right edge to resize it."));
            b.push(Block::new(Kind::Text, ""));
            e.page.blocks = b;
        }
        // A sample picture, drawn in code so the app needs no downloads.
        let assets = self.assets_dir();
        if fs::create_dir_all(&assets).is_ok() {
            let img = crate::covers::render_pattern(1, 960, 300);
            let mut raw = Vec::with_capacity(img.pixels.len() * 4);
            for p in &img.pixels {
                raw.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
            }
            if let Some(buf) = image::RgbaImage::from_raw(960, 300, raw) {
                let mut png = std::io::Cursor::new(Vec::new());
                let ok = buf.write_to(&mut png, image::ImageFormat::Png).is_ok() && fs::write_atomic(&assets.join("sample-hills.png"), png.get_ref()).is_ok();
                if ok {
                    let e = self.get_mut(&id).unwrap();
                    let mut im = Block::new(Kind::Image, "A picture stored next to your notes");
                    im.extra = "_assets/sample-hills.png".into();
                    im.size = 560;
                    let at = e.page.blocks.len() - 1;
                    e.page.blocks.insert(at, im);
                }
            }
        }
        let id2 = self.create("Weekly Plan", "", now);
        {
            let e = self.get_mut(&id2).unwrap();
            e.page.icon = "🗓".into();
            let mut t = Block::new(Kind::Table, "");
            t.table = vec![
                vec!["Day".into(), "Focus".into(), "Status".into()],
                vec!["Monday".into(), "Planning".into(), "Done".into()],
                vec!["Tuesday".into(), "Writing".into(), "In progress".into()],
                vec!["Wednesday".into(), "Review".into(), "".into()],
            ];
            e.page.blocks = vec![
                Block::new(Kind::Text, "A simple page with a table and a list."),
                t,
                Block::new(Kind::H2, "Ideas"),
                Block::new(Kind::Numbered, "Read one chapter a day"),
                Block::new(Kind::Numbered, "Walk after lunch"),
                Block::new(Kind::Quote, "Small steps every day."),
            ];
        }
        let id3 = self.create("Reading List", "", now);
        self.get_mut(&id3).unwrap().page.icon = "📚".into();
        let sub = self.create("Book Notes", &id3, now);
        self.get_mut(&sub).unwrap().page.icon = "📝".into();
        {
            let e = self.get_mut(&id3).unwrap();
            let mut link = Block::new(Kind::PageLink, "Book Notes");
            link.extra = sub.clone();
            let mut q1 = Block::new(Kind::Quote, "**A short book about habits**\nEasy to read, with one useful idea per chapter.");
            q1.color = 6;
            let mut q2 = Block::new(Kind::Quote, "**A long novel for the holidays**\nSlow start, but worth it.");
            q2.color = 7;
            e.page.blocks = vec![
                Block::new(Kind::Todo, "Pick next month's book"),
                Block::new(Kind::H3, "Recommended"),
                q1,
                q2,
                Block::new(Kind::Bullet, "Ask friends for suggestions"),
                link,
            ];
        }
        self.flush(1e9, true);
    }
}

// ---------------------------------------------------------------- settings

pub struct Settings {
    pub workspace: Option<PathBuf>,
    pub recent: Vec<PathBuf>,
    pub dark: bool,
    pub sidebar_width: f32,
    /// Ask before closing the app after editing (even when everything is saved).
    pub confirm_close: bool,
}

pub fn settings_path() -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from("/.folio/settings.txt");
    }
    let base = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    let dir = base.join("Library/Application Support/Folio");
    #[cfg(target_os = "windows")]
    let dir = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(base).join("Folio");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let dir = base.join(".config/folio");
    dir.join("settings.txt")
}

pub fn default_workspace() -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from("/My Notes");
    }
    let base = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Documents").join("Folio Notes")
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings { workspace: None, recent: Vec::new(), dark: false, sidebar_width: 240.0, confirm_close: true };
        if let Ok(txt) = fs::read_to_string(&settings_path()) {
            for line in txt.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match k {
                        "workspace" => s.workspace = Some(PathBuf::from(v)),
                        "recent" => s.recent.push(PathBuf::from(v)),
                        "dark" => s.dark = v == "true",
                        "sidebar" => s.sidebar_width = v.parse().unwrap_or(240.0),
                        "confirm_close" => s.confirm_close = v != "false",
                        _ => {}
                    }
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let p = settings_path();
        if let Some(d) = p.parent() {
            let _ = fs::create_dir_all(d);
        }
        let mut out = String::new();
        if let Some(w) = &self.workspace {
            out.push_str(&format!("workspace={}\n", w.display()));
        }
        for r in &self.recent {
            out.push_str(&format!("recent={}\n", r.display()));
        }
        out.push_str(&format!("dark={}\nsidebar={}\nconfirm_close={}\n", self.dark, self.sidebar_width, self.confirm_close));
        let _ = fs::write_atomic(&p, out.as_bytes());
    }

    pub fn remember(&mut self, p: &Path) {
        self.recent.retain(|r| r != p);
        self.recent.insert(0, p.to_path_buf());
        self.recent.truncate(6);
        self.workspace = Some(p.to_path_buf());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn seed_save_reload() {
        let dir = std::env::temp_dir().join(format!("folio-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut ws = Workspace::open(dir.clone());
        ws.seed_welcome();
        let n = ws.entries.len();
        assert!(ws.entries.iter().all(|e| !e.dirty && e.path.exists()));
        let todo = ws.entries.iter().find(|e| e.page.title == "To-do").unwrap();
        println!("{}", fs::read_to_string(&todo.path).unwrap());
        let row = ws.entries.iter().find(|e| e.page.title == "Renew passport").unwrap();
        println!("{}", fs::read_to_string(&row.path).unwrap());
        let ws2 = Workspace::open(dir.clone());
        assert_eq!(ws2.entries.len(), n);
        let todo2 = ws2.entries.iter().find(|e| e.page.title == "To-do").unwrap();
        assert_eq!(todo2.page.db.as_ref().unwrap().columns.len(), 5);
        assert_eq!(ws2.children(&todo2.page.id).len(), 8);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn folder_pages() {
        let dir = std::env::temp_dir().join(format!("folio-folder-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut ws = Workspace::open(dir.clone());
        let p = ws.create("Projects", "", 0.0);
        let c = ws.create("Alpha", &p, 0.0);
        let g = ws.create("Notes", &c, 0.0);
        ws.flush(1e9, true);
        assert_eq!(ws.get(&c).unwrap().path, dir.join("Alpha.md"));
        ws.set_folder(&p, true, 0.0);
        ws.flush(1e9, true);
        assert_eq!(ws.get(&p).unwrap().path, dir.join("Projects.md"));
        assert_eq!(ws.get(&c).unwrap().path, dir.join("Projects/Alpha.md"));
        assert_eq!(ws.get(&g).unwrap().path, dir.join("Projects/Notes.md"));
        assert!(!dir.join("Alpha.md").exists());
        // Renaming the folder page renames the folder.
        ws.get_mut(&p).unwrap().page.title = "Work".into();
        ws.touch(&p, 0.0);
        ws.flush(1e9, true);
        assert_eq!(ws.get(&c).unwrap().path, dir.join("Work/Alpha.md"));
        assert!(dir.join("Work/Alpha.md").exists() && !dir.join("Projects").exists());
        // Reloading keeps the tree.
        let ws2 = Workspace::open(dir.clone());
        assert_eq!(ws2.get(&c).unwrap().page.parent, p);
        // Turning it off moves the pages back out and removes the empty folder.
        ws.set_folder(&p, false, 0.0);
        ws.flush(1e9, true);
        assert_eq!(ws.get(&c).unwrap().path, dir.join("Alpha.md"));
        assert!(!dir.join("Work").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_then_import() {
        let a = std::env::temp_dir().join(format!("folio-exp-a-{}", std::process::id()));
        let b = std::env::temp_dir().join(format!("folio-exp-b-{}", std::process::id()));
        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
        let mut ws = Workspace::open(a.clone());
        ws.seed_welcome();
        let p = ws.create("Projects", "", 0.0);
        ws.create("Alpha", &p, 0.0);
        ws.set_folder(&p, true, 0.0);
        ws.flush(1e9, true);
        let n_pages = ws.entries.len();
        let zip = ws.export_zip();
        let mut ws2 = Workspace::open(b.clone());
        let n = ws2.import_files(vec![("My Notes.zip".into(), zip)]).unwrap();
        assert!(n > n_pages, "pages and pictures imported");
        assert_eq!(ws2.entries.len(), n_pages);
        assert!(b.join("Projects/Alpha.md").exists());
        assert!(b.join("_assets/sample-hills.png").exists());
        let alpha = ws2.entries.iter().find(|e| e.page.title == "Alpha").unwrap();
        assert_eq!(alpha.page.parent, p);
        // Importing a single page next to an existing one with the same name keeps both.
        ws2.import_files(vec![("Alpha.md".into(), b"# Alpha\n\nanother".to_vec())]).unwrap();
        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn own_folders_are_kept() {
        let dir = std::env::temp_dir().join(format!("folio-own-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Ideas")).unwrap();
        fs::write(dir.join("Ideas/Child.md"), "# Child page\n\n- one\n").unwrap();
        let mut ws = Workspace::open(dir.clone());
        ws.flush(1e9, true); // gives the plain file its header
        assert!(dir.join("Ideas/Child page.md").exists(), "stays in the person's own folder");
        assert!(!dir.join("Child page.md").exists());
        // A page with "Keep sub-pages in a folder" still gathers its sub-pages.
        let p = ws.create("Projects", "", 0.0);
        let c = ws.create("Alpha", &p, 0.0);
        ws.set_folder(&p, true, 0.0);
        ws.flush(1e9, true);
        assert_eq!(ws.get(&c).unwrap().path, dir.join("Projects/Alpha.md"));
        ws.set_folder(&p, false, 0.0);
        ws.flush(1e9, true);
        assert_eq!(ws.get(&c).unwrap().path, dir.join("Alpha.md"));
        let _ = fs::remove_dir_all(&dir);
    }
}
// ------------------------------------------------------------------ cloud sync folders (desktop)

/// The cloud services Folio can store notes with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cloud {
    Dropbox,
    OneDrive,
    GoogleDrive,
}

impl Cloud {
    pub const ALL: [Cloud; 3] = [Cloud::Dropbox, Cloud::OneDrive, Cloud::GoogleDrive];
    pub fn name(self) -> &'static str {
        match self {
            Cloud::Dropbox => "Dropbox",
            Cloud::OneDrive => "OneDrive",
            Cloud::GoogleDrive => "Google Drive",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Cloud::Dropbox => "dropbox",
            Cloud::OneDrive => "onedrive",
            Cloud::GoogleDrive => "google",
        }
    }
    /// Where the web app keeps the notes inside the service, so the desktop app
    /// and the web app use the same folder.
    pub fn notes_subfolder(self) -> &'static str {
        match self {
            Cloud::Dropbox | Cloud::OneDrive => "Apps/Folio",
            Cloud::GoogleDrive => "Folio Notes",
        }
    }
}

/// The folder the service's desktop app syncs (Dropbox, OneDrive, Google Drive for desktop).
pub fn cloud_sync_root(cloud: Cloud) -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)?;
    let storage = home.join("Library/CloudStorage");
    let in_storage = |prefix: &str| -> Option<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(&storage)
            .ok()?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
            .map(|e| e.path())
            .collect();
        found.sort();
        found.into_iter().next()
    };
    let existing = |p: PathBuf| if p.is_dir() { Some(p) } else { None };
    match cloud {
        Cloud::Dropbox => in_storage("Dropbox").or_else(|| existing(home.join("Dropbox"))),
        Cloud::OneDrive => in_storage("OneDrive")
            .or_else(|| std::env::var_os("OneDrive").map(PathBuf::from).and_then(existing))
            .or_else(|| existing(home.join("OneDrive"))),
        Cloud::GoogleDrive => {
            // macOS: ~/Library/CloudStorage/GoogleDrive-<account>/My Drive (name is translated
            // in some languages, e.g. "Mi unidad"); Windows: G:\My Drive.
            let account = in_storage("GoogleDrive-");
            if let Some(acc) = account {
                let mut dirs: Vec<PathBuf> = std::fs::read_dir(&acc).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
                dirs.sort();
                let preferred = ["My Drive", "Mi unidad", "Meine Ablage", "Mon Drive", "Il mio Drive", "Meu Drive"];
                return dirs
                    .iter()
                    .find(|d| preferred.iter().any(|n| d.file_name().map(|f| f == *n).unwrap_or(false)))
                    .cloned()
                    .or_else(|| dirs.into_iter().find(|d| !d.file_name().map(|f| f.to_string_lossy().starts_with('.')).unwrap_or(true)));
            }
            existing(PathBuf::from("G:\\My Drive")).or_else(|| existing(home.join("Google Drive")))
        }
    }
}

/// Copy every file of one notes folder into another (used when moving the notes
/// into an empty cloud folder).
pub fn copy_notes(from: &Path, to: &Path) -> usize {
    let mut n = 0;
    for p in fs::list_all(from) {
        if let (Ok(rel), Ok(bytes)) = (p.strip_prefix(from), fs::read(&p)) {
            if fs::write_atomic(&to.join(rel), &bytes).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// The notes folder inside a cloud service's synced folder (created if needed).
pub fn cloud_notes_folder(cloud: Cloud) -> Option<PathBuf> {
    let root = cloud_sync_root(cloud)?;
    let dir = root.join(cloud.notes_subfolder());
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
