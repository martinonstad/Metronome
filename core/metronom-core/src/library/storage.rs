//! Where the library's files live. The library only ever talks to a [`Storage`], so it can be
//! tested in memory and later pointed at a different folder without touching the parsers.
//!
//! Paths are relative and `/`-separated (`songs.md`, `setlists/friday-gig.md`).

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub trait Storage {
    /// The file's bytes, or `None` if it does not exist.
    fn read(&self, path: &str) -> io::Result<Option<Vec<u8>>>;

    /// Replace the file with `data` so that a crash can never leave it half-written. Folders
    /// are created as needed.
    fn write(&self, path: &str, data: &[u8]) -> io::Result<()>;

    /// Delete the file. Deleting a file that does not exist is not an error.
    fn delete(&self, path: &str) -> io::Result<()>;

    /// The names of the files directly inside `dir` (not folders), sorted. A folder that does
    /// not exist is empty.
    fn list(&self, dir: &str) -> io::Result<Vec<String>>;
}

/// Reject anything that could escape the library folder or is not a plain relative path.
pub fn check_path(path: &str) -> io::Result<()> {
    let bad = path.is_empty()
        || path.len() > 255
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a valid library path: {path:?}"),
        ))
    } else {
        Ok(())
    }
}

/// Files in memory: for tests, and as the staging area for imports.
#[derive(Default)]
pub struct MemStorage {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    fail_writes_after: Mutex<Option<usize>>,
    writes: AtomicUsize,
}

impl MemStorage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Put a text file in place.
    pub fn with(self, path: &str, text: &str) -> Self {
        self.files
            .lock()
            .expect("lock")
            .insert(path.to_string(), text.as_bytes().to_vec());
        self
    }

    pub fn text(&self, path: &str) -> Option<String> {
        self.files
            .lock()
            .expect("lock")
            .get(path)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }

    pub fn paths(&self) -> Vec<String> {
        self.files.lock().expect("lock").keys().cloned().collect()
    }

    /// How many writes have succeeded so far.
    pub fn write_count(&self) -> usize {
        self.writes.load(Ordering::Relaxed)
    }

    /// Make every write after the next `n` successful ones fail, to test interrupted saves.
    pub fn fail_writes_after(&self, n: usize) {
        *self.fail_writes_after.lock().expect("lock") = Some(n);
    }
}

impl Storage for MemStorage {
    fn read(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        check_path(path)?;
        Ok(self.files.lock().expect("lock").get(path).cloned())
    }

    fn write(&self, path: &str, data: &[u8]) -> io::Result<()> {
        check_path(path)?;
        if let Some(remaining) = self.fail_writes_after.lock().expect("lock").as_mut() {
            if *remaining == 0 {
                return Err(io::Error::other("simulated write failure"));
            }
            *remaining -= 1;
        }
        self.files
            .lock()
            .expect("lock")
            .insert(path.to_string(), data.to_vec());
        self.writes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        check_path(path)?;
        self.files.lock().expect("lock").remove(path);
        Ok(())
    }

    fn list(&self, dir: &str) -> io::Result<Vec<String>> {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            check_path(dir)?;
            format!("{dir}/")
        };
        Ok(self
            .files
            .lock()
            .expect("lock")
            .keys()
            .filter_map(|p| p.strip_prefix(&prefix))
            .filter(|rest| !rest.contains('/'))
            .map(str::to_string)
            .collect())
    }
}

/// Files in a folder on disk. Writes go to a temporary file first and are renamed into place.
pub struct FsStorage {
    root: PathBuf,
}

impl FsStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &str) -> io::Result<PathBuf> {
        check_path(path)?;
        Ok(path
            .split('/')
            .fold(self.root.clone(), |p, part| p.join(part)))
    }
}

impl Storage for FsStorage {
    fn read(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        match std::fs::read(self.resolve(path)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write(&self, path: &str, data: &[u8]) -> io::Result<()> {
        use std::io::Write;
        let target = self.resolve(path)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let name = target
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
        let temp = target.with_file_name(format!(".{name}.tmp"));
        let result = (|| {
            let mut file = std::fs::File::create(&temp)?;
            file.write_all(data)?;
            file.sync_all()?;
            std::fs::rename(&temp, &target)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }

    fn delete(&self, path: &str) -> io::Result<()> {
        match std::fs::remove_file(self.resolve(path)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn list(&self, dir: &str) -> io::Result<Vec<String>> {
        let folder = if dir.is_empty() {
            self.root.clone()
        } else {
            self.resolve(dir)?
        };
        let entries = match std::fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && let Some(name) = entry.file_name().to_str()
            {
                names.push(name.to_string());
            }
        }
        names.sort();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_relative_paths_are_allowed() {
        for ok in ["songs.md", "setlists/friday-gig.md", "a/b/c.md"] {
            assert!(check_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "./x",
            "a\\b",
            "x\0y",
            "a/",
            &"x".repeat(300),
        ] {
            assert!(check_path(bad).is_err(), "{bad:?}");
        }
    }

    fn exercise(storage: &dyn Storage) {
        assert_eq!(storage.read("songs.md").unwrap(), None);
        storage.write("songs.md", b"one").unwrap();
        storage.write("songs.md", b"two").unwrap();
        assert_eq!(
            storage.read("songs.md").unwrap().as_deref(),
            Some(&b"two"[..])
        );
        storage.write("setlists/b.md", b"B").unwrap();
        storage.write("setlists/a.md", b"A").unwrap();
        assert_eq!(storage.list("setlists").unwrap(), ["a.md", "b.md"]);
        assert_eq!(storage.list("nothing").unwrap(), Vec::<String>::new());
        assert!(storage.list("").unwrap().contains(&"songs.md".to_string()));
        assert!(
            !storage.list("").unwrap().contains(&"setlists".to_string()),
            "folders are not files"
        );
        storage.delete("setlists/a.md").unwrap();
        storage.delete("setlists/a.md").unwrap();
        assert_eq!(storage.list("setlists").unwrap(), ["b.md"]);
        assert!(storage.write("../escape.md", b"x").is_err());
        assert!(storage.read("/etc/passwd").is_err());
        assert!(storage.delete("a/../b").is_err());
    }

    #[test]
    fn memory_storage_behaves() {
        exercise(&MemStorage::new());
    }

    #[test]
    fn disk_storage_behaves_and_leaves_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let storage = FsStorage::new(dir.path());
        exercise(&storage);
        for entry in std::fs::read_dir(dir.path())
            .unwrap()
            .chain(std::fs::read_dir(dir.path().join("setlists")).unwrap())
        {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(!name.ends_with(".tmp"), "{name}");
        }
    }

    #[test]
    fn a_failed_disk_write_leaves_the_old_file_intact() {
        let dir = tempfile::tempdir().unwrap();
        let storage = FsStorage::new(dir.path());
        storage.write("songs.md", b"original").unwrap();
        // A folder where the temporary file would go makes the write fail part-way.
        std::fs::create_dir(dir.path().join(".songs.md.tmp")).unwrap();
        assert!(storage.write("songs.md", b"new").is_err());
        assert_eq!(
            storage.read("songs.md").unwrap().as_deref(),
            Some(&b"original"[..])
        );
    }

    #[test]
    fn memory_storage_can_simulate_an_interrupted_save() {
        let storage = MemStorage::new();
        storage.fail_writes_after(1);
        assert!(storage.write("a.md", b"1").is_ok());
        assert!(storage.write("b.md", b"2").is_err());
        assert_eq!(storage.paths(), ["a.md"]);
    }
}
