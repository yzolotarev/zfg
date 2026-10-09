use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use super::status::Status;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    Focus,
    Later,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub focus: Vec<String>,
    pub later: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddFocusResult {
    Added,
    Full { current: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
    focus_limit: usize,
}

impl Store {
    pub fn new(root: PathBuf, focus_limit: usize) -> Result<Self> {
        if focus_limit == 0 {
            bail!("focus capacity must be greater than zero");
        }
        fs::create_dir_all(root.join("state"))?;
        // Persist the state directory entry before creating recovery metadata in it.
        sync_dir(&root)?;
        let store = Self { root, focus_limit };
        store.locked(|| {
            for name in ["focus.txt", "later.txt"] {
                let path = store.root.join(name);
                match fs::symlink_metadata(&path) {
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        atomic_write(&path, b"")?;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Ok(())
        })?;
        Ok(store)
    }

    pub fn data_dir(&self) -> &Path {
        &self.root
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        self.locked(|| self.read())
    }

    pub fn status(&self) -> Result<Status> {
        let snapshot = self.snapshot()?;
        Ok(Status {
            focus_count: snapshot.focus.len(),
            focus_limit: self.focus_limit,
            focus_first: snapshot.focus.first().cloned().unwrap_or_default(),
            later_count: snapshot.later.len(),
            focus_full: snapshot.focus.len() >= self.focus_limit,
        })
    }

    pub fn add_focus(&self, text: &str) -> Result<AddFocusResult> {
        let text = normalize(text)?;
        self.locked(|| {
            let mut current = self.read()?;
            if current.focus.len() >= self.focus_limit {
                return Ok(AddFocusResult::Full {
                    current: current.focus,
                });
            }
            current.focus.push(text);
            self.write_list(List::Focus, &current.focus)?;
            Ok(AddFocusResult::Added)
        })
    }

    pub fn add_later(&self, text: &str) -> Result<()> {
        let text = normalize(text)?;
        self.locked(|| {
            let mut current = self.read()?;
            current.later.push(text);
            self.write_list(List::Later, &current.later)
        })
    }

    pub fn replace_focus(&self, index: usize, new_task: &str, snapshot: &Snapshot) -> Result<()> {
        let text = normalize(new_task)?;
        self.locked(|| {
            let mut current = self.checked(snapshot)?;
            check_index(&current.focus, index)?;
            let displaced = std::mem::replace(&mut current.focus[index], text);
            current.later.push(displaced);
            self.commit_transaction(&current)
        })
    }

    pub fn replace_focus_from_later(
        &self,
        focus_index: usize,
        later_index: usize,
        snapshot: &Snapshot,
    ) -> Result<()> {
        self.locked(|| {
            let mut current = self.checked(snapshot)?;
            check_index(&current.focus, focus_index)?;
            check_index(&current.later, later_index)?;
            let incoming = current.later.remove(later_index);
            let displaced = std::mem::replace(&mut current.focus[focus_index], incoming);
            current.later.push(displaced);
            self.commit_transaction(&current)
        })
    }

    pub fn complete(&self, list: List, index: usize, snapshot: &Snapshot) -> Result<()> {
        self.mutate(snapshot, list, |tasks| {
            check_index(tasks, index)?;
            tasks.remove(index);
            Ok(())
        })
    }

    /// Moves the indexed task to the end of the other list.
    pub fn move_task(
        &self,
        list: List,
        index: usize,
        snapshot: &Snapshot,
    ) -> Result<AddFocusResult> {
        self.locked(|| {
            let mut current = self.checked(snapshot)?;
            check_index(tasks_mut(&mut current, list), index)?;
            if list == List::Later && current.focus.len() >= self.focus_limit {
                return Ok(AddFocusResult::Full {
                    current: current.focus,
                });
            }
            let task = tasks_mut(&mut current, list).remove(index);
            let target = match list {
                List::Focus => List::Later,
                List::Later => List::Focus,
            };
            tasks_mut(&mut current, target).push(task);
            self.commit_transaction(&current)?;
            Ok(AddFocusResult::Added)
        })
    }

    pub fn edit(&self, list: List, index: usize, text: &str, snapshot: &Snapshot) -> Result<()> {
        let text = normalize(text)?;
        self.mutate(snapshot, list, |tasks| {
            check_index(tasks, index)?;
            tasks[index] = text;
            Ok(())
        })
    }

    /// Both positions refer to the original list; `to` is the final position.
    pub fn reorder_focus(&self, from: usize, to: usize, snapshot: &Snapshot) -> Result<()> {
        self.mutate(snapshot, List::Focus, |tasks| {
            check_index(tasks, from)?;
            check_index(tasks, to)?;
            let task = tasks.remove(from);
            tasks.insert(to, task);
            Ok(())
        })
    }

    fn mutate(
        &self,
        snapshot: &Snapshot,
        list: List,
        action: impl FnOnce(&mut Vec<String>) -> Result<()>,
    ) -> Result<()> {
        self.locked(|| {
            let mut current = self.checked(snapshot)?;
            let tasks = tasks_mut(&mut current, list);
            action(tasks)?;
            self.write_list(list, tasks)
        })
    }

    fn checked(&self, snapshot: &Snapshot) -> Result<Snapshot> {
        let current = self.read()?;
        if &current != snapshot {
            bail!("stale snapshot: task lists have changed");
        }
        Ok(current)
    }

    fn locked<T>(&self, action: impl FnOnce() -> Result<T>) -> Result<T> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join("state/store.lock"))?;
        FileExt::lock_exclusive(&lock).context("locking task store")?;
        // The file's lifetime releases the lock even on an error or panic.
        self.recover()?;
        action()
    }

    fn recover(&self) -> Result<()> {
        let path = self.root.join("state/txn.json");
        match fs::read(&path) {
            Ok(bytes) => {
                let transaction: Snapshot = serde_json::from_slice(&bytes)
                    .context("invalid recovery journal; leaving it intact")?;
                self.apply_transaction(&transaction)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn commit_transaction(&self, transaction: &Snapshot) -> Result<()> {
        // A redo journal stores the complete target state, so replay is idempotent.
        let bytes = serde_json::to_vec(transaction)?;
        atomic_write(&self.root.join("state/txn.json"), &bytes)?;
        self.apply_transaction(transaction)
    }

    fn apply_transaction(&self, transaction: &Snapshot) -> Result<()> {
        self.write_list(List::Focus, &transaction.focus)?;
        self.write_list(List::Later, &transaction.later)?;
        fs::remove_file(self.root.join("state/txn.json"))?;
        sync_dir(&self.root.join("state"))
    }

    fn read(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            focus: read_list(&self.root.join("focus.txt"))?,
            later: read_list(&self.root.join("later.txt"))?,
        })
    }

    fn write_list(&self, list: List, tasks: &[String]) -> Result<()> {
        let name = match list {
            List::Focus => "focus.txt",
            List::Later => "later.txt",
        };
        let mut text = tasks.join("\n");
        if !tasks.is_empty() {
            text.push('\n');
        }
        atomic_write(&self.root.join(name), text.as_bytes())
    }
}

fn tasks_mut(snapshot: &mut Snapshot, list: List) -> &mut Vec<String> {
    match list {
        List::Focus => &mut snapshot.focus,
        List::Later => &mut snapshot.later,
    }
}

fn check_index(tasks: &[String], index: usize) -> Result<()> {
    if index >= tasks.len() {
        bail!(
            "task index {index} is out of bounds for {} tasks",
            tasks.len()
        );
    }
    Ok(())
}

fn normalize(text: &str) -> Result<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        bail!("task must not be empty");
    }
    if text.chars().count() > 500 {
        bail!("task exceeds 500 characters");
    }
    Ok(text)
}

fn read_list(path: &Path) -> Result<Vec<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("storage path has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync_dir(parent)
}

fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
