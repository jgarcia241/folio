//! The files the app reads and writes.
//!
//! * Desktop: the real folder on disk (`std::fs`).
//! * Web / installed PWA: an in-memory copy of the folder that the page's
//!   script loads from the browser's storage (IndexedDB) at start-up and saves
//!   back after every change, so the app works fully offline.
//!
//! Every path is an ordinary `Path`; on the web they look like `/My Notes/Page.md`.

use std::io;
use std::path::{Path, PathBuf};

pub fn now_millis() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A page file and when it last changed (milliseconds).
pub type Listed = (PathBuf, u64);

fn skip_name(name: &str) -> bool {
    name.starts_with('.') || name.starts_with('_')
}

// ------------------------------------------------------------------ desktop

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;
    use std::fs;

    fn mtime_of(m: &fs::Metadata) -> u64 {
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn read(p: &Path) -> io::Result<Vec<u8>> {
        fs::read(p)
    }
    pub fn write_atomic(p: &Path, data: &[u8]) -> io::Result<()> {
        let dir = p.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let tmp = dir.join(format!(".{name}.tmp"));
        fs::write(&tmp, data)?;
        fs::rename(&tmp, p).inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }
    pub fn remove_file(p: &Path) -> io::Result<()> {
        fs::remove_file(p)
    }
    pub fn rename(a: &Path, b: &Path) -> io::Result<()> {
        fs::rename(a, b)
    }
    pub fn exists(p: &Path) -> bool {
        p.exists()
    }
    pub fn is_dir(p: &Path) -> bool {
        p.is_dir()
    }
    pub fn create_dir_all(p: &Path) -> io::Result<()> {
        fs::create_dir_all(p)
    }
    pub fn remove_dir_if_empty(p: &Path) -> bool {
        let empty = fs::read_dir(p).map(|mut r| r.next().is_none()).unwrap_or(false);
        empty && fs::remove_dir(p).is_ok()
    }
    pub fn remove_dir_all(p: &Path) {
        let _ = fs::remove_dir_all(p);
    }
    pub fn len(p: &Path) -> Option<u64> {
        fs::metadata(p).ok().map(|m| m.len())
    }
    pub fn mtime(p: &Path) -> u64 {
        fs::metadata(p).map(|m| mtime_of(&m)).unwrap_or(0)
    }
    /// Every file under `root` (relative paths kept), for export.
    pub fn list_all(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let p = e.path();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(p);
                } else {
                    out.push(p);
                }
            }
        }
        out
    }
    pub fn list_md(root: &Path) -> Vec<Listed> {
        let mut v = Vec::new();
        let mut stack = vec![(root.to_path_buf(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if skip_name(&name) {
                    continue;
                }
                let p = e.path();
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() {
                    if depth < 12 {
                        stack.push((p, depth + 1));
                    }
                } else if p.extension().map(|x| x == "md").unwrap_or(false) {
                    let mt = e.metadata().map(|m| mtime_of(&m)).unwrap_or(0);
                    v.push((p, mt));
                }
            }
        }
        v
    }
}

// ------------------------------------------------------------------ web

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    thread_local! {
        static FILES: RefCell<BTreeMap<String, (Vec<u8>, u64)>> = RefCell::new(BTreeMap::new());
    }

    fn key(p: &Path) -> String {
        let s = p.to_string_lossy().replace('\\', "/");
        let s = if s.starts_with('/') { s } else { format!("/{s}") };
        s.trim_end_matches('/').to_string()
    }

    /// Called by the page's script before the app starts.
    pub fn preload(path: String, data: Vec<u8>, mtime: u64) {
        FILES.with(|f| f.borrow_mut().insert(key(Path::new(&path)), (data, mtime)));
    }

    /// A file changed outside the app (in the linked folder): update the copy
    /// without telling the page's script again.
    pub fn external_put(path: String, data: Vec<u8>, mtime: u64) {
        preload(path, data, mtime);
    }

    pub fn external_remove(path: &str) {
        FILES.with(|f| f.borrow_mut().remove(&key(Path::new(path))));
    }

    /// Replace everything under `root` with `files` (paths relative to root),
    /// quietly: the page's script stores the same files itself.
    pub fn replace_root(root: &Path, files: Vec<(String, Vec<u8>, u64)>) {
        let pre = format!("{}/", key(root));
        FILES.with(|f| {
            let mut f = f.borrow_mut();
            f.retain(|k, _| !k.starts_with(&pre));
            for (rel, data, mt) in files {
                f.insert(format!("{pre}{}", rel.trim_start_matches('/')), (data, mt));
            }
        });
    }

    /// Replace a file from the outside (import) and save it.
    pub fn read(p: &Path) -> io::Result<Vec<u8>> {
        FILES.with(|f| f.borrow().get(&key(p)).map(|(d, _)| d.clone())).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not found"))
    }
    pub fn write_atomic(p: &Path, data: &[u8]) -> io::Result<()> {
        let k = key(p);
        let mt = now_millis();
        crate::platform::host_put(&k, data, mt as f64);
        FILES.with(|f| f.borrow_mut().insert(k, (data.to_vec(), mt)));
        Ok(())
    }
    pub fn remove_file(p: &Path) -> io::Result<()> {
        let k = key(p);
        let had = FILES.with(|f| f.borrow_mut().remove(&k)).is_some();
        if had {
            crate::platform::host_remove(&k);
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
        }
    }
    pub fn rename(a: &Path, b: &Path) -> io::Result<()> {
        let (ka, kb) = (key(a), key(b));
        let moved: Vec<(String, (Vec<u8>, u64))> = FILES.with(|f| {
            let f = f.borrow();
            f.iter()
                .filter(|(k, _)| **k == ka || k.starts_with(&format!("{ka}/")))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        });
        if moved.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
        }
        for (k, (data, _)) in moved {
            let nk = format!("{kb}{}", &k[ka.len()..]);
            FILES.with(|f| f.borrow_mut().remove(&k));
            crate::platform::host_remove(&k);
            write_atomic(Path::new(&nk), &data)?;
        }
        Ok(())
    }
    pub fn exists(p: &Path) -> bool {
        let k = key(p);
        FILES.with(|f| f.borrow().contains_key(&k)) || is_dir(p)
    }
    pub fn is_dir(p: &Path) -> bool {
        let pre = format!("{}/", key(p));
        FILES.with(|f| f.borrow().range(pre.clone()..).next().map(|(k, _)| k.starts_with(&pre)).unwrap_or(false))
    }
    pub fn create_dir_all(_p: &Path) -> io::Result<()> {
        Ok(()) // folders exist implicitly
    }
    pub fn remove_dir_if_empty(p: &Path) -> bool {
        !is_dir(p)
    }
    pub fn remove_dir_all(p: &Path) {
        let pre = format!("{}/", key(p));
        let ks: Vec<String> = FILES.with(|f| f.borrow().keys().filter(|k| k.starts_with(&pre)).cloned().collect());
        for k in ks {
            let _ = remove_file(Path::new(&k));
        }
    }
    pub fn len(p: &Path) -> Option<u64> {
        FILES.with(|f| f.borrow().get(&key(p)).map(|(d, _)| d.len() as u64))
    }
    pub fn mtime(p: &Path) -> u64 {
        FILES.with(|f| f.borrow().get(&key(p)).map(|(_, m)| *m)).unwrap_or(0)
    }
    pub fn list_all(root: &Path) -> Vec<PathBuf> {
        let pre = format!("{}/", key(root));
        FILES.with(|f| f.borrow().keys().filter(|k| k.starts_with(&pre)).map(PathBuf::from).collect())
    }
    pub fn list_md(root: &Path) -> Vec<Listed> {
        let pre = format!("{}/", key(root));
        FILES.with(|f| {
            f.borrow()
                .iter()
                .filter(|(k, _)| k.starts_with(&pre) && k.ends_with(".md"))
                .filter(|(k, _)| !k[pre.len()..].split('/').any(skip_name))
                .map(|(k, (_, m))| (PathBuf::from(k), *m))
                .collect()
        })
    }
}

pub use imp::*;

pub fn read_to_string(p: &Path) -> io::Result<String> {
    let b = read(p)?;
    String::from_utf8(b).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
