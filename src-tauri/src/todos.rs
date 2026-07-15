//! Project-scoped todo queue (spec `docs/specs/todo-whats-next.md` D1).
//!
//! `~/.askhuman/state/todos.json` is the single source of truth: every process reads and
//! writes the file directly (no daemon-resident state — todos have no hot path). Mutations
//! take an exclusive advisory lock (`todos.lock`, same pattern as the history write lock;
//! best-effort no-op off Unix) around the read-modify-write, and the file itself is written
//! atomically (tmp + rename). Empty project keys are pruned on write.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One pending todo entry. FIFO order is the `Vec` order in the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoEntry {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub created_at_ms: u64,
}

/// On-disk shape: project key (git root path) → FIFO entries.
#[derive(Default, Serialize, Deserialize)]
struct TodoFile {
    #[serde(default)]
    projects: HashMap<String, Vec<TodoEntry>>,
}

fn todos_file() -> PathBuf {
    crate::paths::state_dir().join("todos.json")
}

fn todos_lock() -> PathBuf {
    crate::paths::state_dir().join("todos.lock")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn load_at(path: &Path) -> TodoFile {
    let Ok(text) = std::fs::read_to_string(path) else {
        return TodoFile::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Atomic write; prunes projects with no entries. Best-effort (failure is silent, the queue
/// simply keeps its previous on-disk state).
fn store_at(path: &Path, mut data: TodoFile) {
    data.projects.retain(|_, entries| !entries.is_empty());
    let Ok(json) = serde_json::to_string_pretty(&data) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension(format!("json.tmp-{}", uuid::Uuid::new_v4()));
    if std::fs::write(&tmp, json.as_bytes()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

// ===== Cross-process write lock (same pattern as history.rs) =====

#[cfg(unix)]
struct LockGuard {
    _file: std::fs::File,
}

#[cfg(unix)]
fn lock_at(path: &Path) -> Option<LockGuard> {
    use std::os::unix::io::AsRawFd;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .ok()?;
    unsafe {
        libc::flock(file.as_raw_fd(), libc::LOCK_EX);
    }
    Some(LockGuard { _file: file })
}

#[cfg(not(unix))]
fn lock_at(_path: &Path) -> Option<()> {
    None
}

/// Normalize a project key/text pair for storage; `None` when unusable.
fn normalized(project: &str, text: &str) -> Option<(String, String)> {
    let project = project.trim();
    let text = text.trim();
    (!project.is_empty() && !text.is_empty()).then(|| (project.to_string(), text.to_string()))
}

// ===== Public API (default paths) =====

/// Pending todos of a project (FIFO). Missing file / unknown project → empty.
pub fn list(project: &str) -> Vec<TodoEntry> {
    list_at(&todos_file(), project)
}

/// Full snapshot: project key → entries (GUI window / project selector).
pub fn all() -> HashMap<String, Vec<TodoEntry>> {
    load_at(&todos_file()).projects
}

/// Append one entry; returns it (or `None` when project/text is empty after trim).
pub fn add(project: &str, text: &str) -> Option<TodoEntry> {
    add_at(&todos_file(), &todos_lock(), project, text)
}

/// Remove one entry by id. Returns whether it existed.
pub fn remove(project: &str, id: &str) -> bool {
    remove_at(&todos_file(), &todos_lock(), project, id)
}

/// Clear a project's queue; returns how many entries were removed.
pub fn clear(project: &str) -> usize {
    clear_at(&todos_file(), &todos_lock(), project)
}

/// Dequeue entries by id (best-effort: missing ids are skipped, spec D11). Returns the
/// entries actually removed. This is the "started executing → auto-clear" point.
pub fn take(project: &str, ids: &[String]) -> Vec<TodoEntry> {
    take_at(&todos_file(), &todos_lock(), project, ids)
}

/// Collect the todo ids a terminal answer consumed (pure function, spec D2/D5/D7).
///
/// Two sources, deduplicated:
/// - options carrying a `todo_id` whose text was selected (whats-next / Stop-card chips;
///   channels only report the option text, so ids are recovered from the request);
/// - explicit `QuestionAnswer.todo_ids` (popup collapsible todo section).
///
/// The caller (Coordinator, at the first-terminal convergence point) passes the result to
/// [`take`]; missing ids are skipped there (best-effort, spec D11).
pub fn ids_to_dequeue(
    request: &crate::models::AskRequest,
    result: &crate::models::ChannelResult,
) -> Vec<String> {
    if result.action != crate::models::ChannelAction::Send {
        return Vec::new();
    }
    let mut ids: Vec<String> = Vec::new();
    for (i, answer) in result.answers.iter().enumerate() {
        let options = request
            .questions
            .get(i)
            .map(|q| q.predefined_options.as_slice())
            .unwrap_or(&[]);
        for sel in &answer.selected_options {
            if let Some(id) = options
                .iter()
                .find(|o| &o.text == sel)
                .and_then(|o| o.todo_id.clone())
            {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        for id in &answer.todo_ids {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

// ===== Path-parameterized implementations (unit-testable without touching the real home) =====

pub fn list_at(path: &Path, project: &str) -> Vec<TodoEntry> {
    load_at(path)
        .projects
        .get(project.trim())
        .cloned()
        .unwrap_or_default()
}

pub fn add_at(path: &Path, lock: &Path, project: &str, text: &str) -> Option<TodoEntry> {
    let (project, text) = normalized(project, text)?;
    let _guard = lock_at(lock);
    let mut data = load_at(path);
    let entry = TodoEntry {
        id: uuid::Uuid::new_v4().to_string(),
        text,
        created_at_ms: now_ms(),
    };
    data.projects.entry(project).or_default().push(entry.clone());
    store_at(path, data);
    Some(entry)
}

pub fn remove_at(path: &Path, lock: &Path, project: &str, id: &str) -> bool {
    let _guard = lock_at(lock);
    let mut data = load_at(path);
    let Some(entries) = data.projects.get_mut(project.trim()) else {
        return false;
    };
    let before = entries.len();
    entries.retain(|e| e.id != id);
    let removed = entries.len() != before;
    if removed {
        store_at(path, data);
    }
    removed
}

pub fn clear_at(path: &Path, lock: &Path, project: &str) -> usize {
    let _guard = lock_at(lock);
    let mut data = load_at(path);
    let removed = data
        .projects
        .remove(project.trim())
        .map(|e| e.len())
        .unwrap_or(0);
    if removed > 0 {
        store_at(path, data);
    }
    removed
}

pub fn take_at(path: &Path, lock: &Path, project: &str, ids: &[String]) -> Vec<TodoEntry> {
    if ids.is_empty() {
        return Vec::new();
    }
    let _guard = lock_at(lock);
    let mut data = load_at(path);
    let Some(entries) = data.projects.get_mut(project.trim()) else {
        return Vec::new();
    };
    let mut taken = Vec::new();
    entries.retain(|e| {
        if ids.iter().any(|id| id == &e.id) {
            taken.push(e.clone());
            false
        } else {
            true
        }
    });
    if !taken.is_empty() {
        store_at(path, data);
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempStore {
        dir: PathBuf,
    }

    impl TempStore {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("ah-todos-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }
        fn file(&self) -> PathBuf {
            self.dir.join("todos.json")
        }
        fn lock(&self) -> PathBuf {
            self.dir.join("todos.lock")
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn add_list_fifo_roundtrip() {
        let t = TempStore::new();
        assert!(list_at(&t.file(), "/p").is_empty());
        let a = add_at(&t.file(), &t.lock(), "/p", "第一条").unwrap();
        let b = add_at(&t.file(), &t.lock(), "/p", "  second  ").unwrap();
        assert_eq!(b.text, "second"); // trimmed
        let entries = list_at(&t.file(), "/p");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, a.id); // FIFO order preserved
        assert_eq!(entries[0].text, "第一条");
        assert!(entries[0].created_at_ms > 0);
        // Other projects unaffected.
        assert!(list_at(&t.file(), "/q").is_empty());
    }

    #[test]
    fn add_rejects_empty_text_or_project() {
        let t = TempStore::new();
        assert!(add_at(&t.file(), &t.lock(), "/p", "   ").is_none());
        assert!(add_at(&t.file(), &t.lock(), "  ", "task").is_none());
        assert!(!t.file().exists());
    }

    #[test]
    fn remove_by_id_and_prune_empty_project() {
        let t = TempStore::new();
        let a = add_at(&t.file(), &t.lock(), "/p", "one").unwrap();
        assert!(remove_at(&t.file(), &t.lock(), "/p", &a.id));
        assert!(!remove_at(&t.file(), &t.lock(), "/p", &a.id)); // already gone
        assert!(list_at(&t.file(), "/p").is_empty());
        // Project key pruned from file.
        let raw = std::fs::read_to_string(t.file()).unwrap();
        assert!(!raw.contains("/p"));
    }

    #[test]
    fn clear_returns_count_and_needs_entries() {
        let t = TempStore::new();
        add_at(&t.file(), &t.lock(), "/p", "a");
        add_at(&t.file(), &t.lock(), "/p", "b");
        assert_eq!(clear_at(&t.file(), &t.lock(), "/p"), 2);
        assert_eq!(clear_at(&t.file(), &t.lock(), "/p"), 0);
    }

    #[test]
    fn take_dequeues_best_effort() {
        let t = TempStore::new();
        let a = add_at(&t.file(), &t.lock(), "/p", "a").unwrap();
        let b = add_at(&t.file(), &t.lock(), "/p", "b").unwrap();
        // One real id + one stale id: only the real one is taken, no error (spec D11).
        let taken = take_at(
            &t.file(),
            &t.lock(),
            "/p",
            &[a.id.clone(), "missing".to_string()],
        );
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].text, "a");
        let left = list_at(&t.file(), "/p");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, b.id);
        // Empty ids → no-op.
        assert!(take_at(&t.file(), &t.lock(), "/p", &[]).is_empty());
        // Unknown project → no-op.
        assert!(take_at(&t.file(), &t.lock(), "/q", &[b.id.clone()]).is_empty());
    }

    #[test]
    fn ids_to_dequeue_collects_selected_chips_and_explicit_ids() {
        use crate::models::{
            AskRequest, ChannelAction, ChannelResult, MessagePrompt, OptionItem, Question,
            QuestionAnswer,
        };
        let request = AskRequest::new(
            MessagePrompt::default(),
            vec![Question::new(
                "What should we do next?".into(),
                vec![
                    OptionItem::with_todo("修 bug", "id-1"),
                    OptionItem::with_todo("写文档", "id-2"),
                    OptionItem::new("End this turn", false),
                ],
            )],
            true,
        );
        // 选中一条待办 chip + 弹窗折叠区显式 id（含重复）→ 去重合并；「结束」选项无 id。
        let result = ChannelResult {
            action: ChannelAction::Send,
            answers: vec![QuestionAnswer {
                selected_options: vec!["修 bug".into(), "End this turn".into()],
                user_input: None,
                images: Vec::new(),
                files: Vec::new(),
                todo_ids: vec!["id-1".into(), "id-3".into()],
            }],
            source_channel_id: "popup".into(),
        };
        assert_eq!(ids_to_dequeue(&request, &result), vec!["id-1", "id-3"]);
        // 取消路径不出队。
        assert!(ids_to_dequeue(&request, &ChannelResult::cancel("popup")).is_empty());
        // 普通提问（无 todo 选项、无显式 id）不出队。
        let plain = ChannelResult {
            action: ChannelAction::Send,
            answers: vec![QuestionAnswer {
                selected_options: vec!["End this turn".into()],
                ..Default::default()
            }],
            source_channel_id: "popup".into(),
        };
        assert!(ids_to_dequeue(&request, &plain).is_empty());
    }

    #[test]
    fn corrupt_file_degrades_to_empty() {
        let t = TempStore::new();
        std::fs::write(t.file(), "not json").unwrap();
        assert!(list_at(&t.file(), "/p").is_empty());
        // Mutation on top of a corrupt file starts fresh instead of failing.
        add_at(&t.file(), &t.lock(), "/p", "x").unwrap();
        assert_eq!(list_at(&t.file(), "/p").len(), 1);
    }

    #[test]
    fn all_snapshot_groups_by_project() {
        let t = TempStore::new();
        add_at(&t.file(), &t.lock(), "/p", "a");
        add_at(&t.file(), &t.lock(), "/q", "b");
        let all = load_at(&t.file()).projects;
        assert_eq!(all.len(), 2);
        assert_eq!(all["/p"][0].text, "a");
        assert_eq!(all["/q"][0].text, "b");
    }
}
