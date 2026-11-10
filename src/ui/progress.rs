//! Saves and restores reading positions without blocking the GTK main thread.
//!
//! Saves are debounced, then handed to a single worker thread so they hit the
//! database in order. Loads run on the GIO worker pool. [`ProgressSync::shutdown`]
//! flushes the last position and waits for the worker, so nothing is lost on exit.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use greenread::storage::{default_database_path, ProgressStore, ReadingPosition};
use gtk::{gio, glib};

const LOG_DOMAIN: &str = "greenread";
const SAVE_DELAY: Duration = Duration::from_millis(500);

struct SaveRequest {
    path: PathBuf,
    position: ReadingPosition,
}

type SharedStore = Arc<Mutex<ProgressStore>>;

pub struct ProgressSync {
    /// `None` when no database could be opened; then every call is a no-op.
    store: Option<SharedStore>,
    saves: RefCell<Option<mpsc::Sender<SaveRequest>>>,
    worker: RefCell<Option<JoinHandle<()>>>,
    pending: RefCell<Option<SaveRequest>>,
    debounce: RefCell<Option<glib::SourceId>>,
}

impl ProgressSync {
    /// Opens the default database. Falls back to an in-memory one (positions
    /// are then kept only for this session) and logs a warning on failure.
    pub fn open_default() -> Rc<Self> {
        let path = default_database_path();
        let store = ProgressStore::open(&path).or_else(|err| {
            glib::g_warning!(
                LOG_DOMAIN,
                "cannot open {}: {err}; reading positions will not be kept after exit",
                path.display()
            );
            ProgressStore::open_in_memory()
        });
        match store {
            Ok(store) => Self::new(store),
            Err(err) => {
                glib::g_warning!(LOG_DOMAIN, "reading positions are disabled: {err}");
                Rc::new(Self::with_store(None))
            }
        }
    }

    pub fn new(store: ProgressStore) -> Rc<Self> {
        let this = Self::with_store(Some(Arc::new(Mutex::new(store))));
        this.start_worker();
        Rc::new(this)
    }

    fn with_store(store: Option<SharedStore>) -> Self {
        Self {
            store,
            saves: RefCell::new(None),
            worker: RefCell::new(None),
            pending: RefCell::new(None),
            debounce: RefCell::new(None),
        }
    }

    fn start_worker(&self) {
        let Some(store) = self.store.clone() else {
            return;
        };
        let (sender, receiver) = mpsc::channel::<SaveRequest>();
        let spawned = thread::Builder::new()
            .name("greenread-progress".into())
            .spawn(move || {
                for request in receiver {
                    save_blocking(&store, &request);
                }
            });
        match spawned {
            Ok(handle) => {
                self.saves.replace(Some(sender));
                self.worker.replace(Some(handle));
            }
            Err(err) => glib::g_warning!(LOG_DOMAIN, "cannot start the save thread: {err}"),
        }
    }

    /// Returns the saved position for `path`, reading the database off the main thread.
    pub async fn load(&self, path: PathBuf) -> Option<ReadingPosition> {
        let store = self.store.clone()?;
        let result = gio::spawn_blocking(move || {
            let store = store
                .lock()
                .map_err(|_| "the store lock is poisoned".to_owned())?;
            store.load(&path).map_err(|err| err.to_string())
        })
        .await;
        match result {
            Ok(Ok(position)) => position,
            Ok(Err(err)) => {
                glib::g_warning!(LOG_DOMAIN, "cannot load reading position: {err}");
                None
            }
            Err(_panic) => {
                glib::g_warning!(LOG_DOMAIN, "loading the reading position panicked");
                None
            }
        }
    }

    /// Remembers `position` and saves it after a short pause, so rapid
    /// scrolling produces one write instead of hundreds.
    pub fn record(self: &Rc<Self>, path: PathBuf, position: ReadingPosition) {
        self.set_pending(SaveRequest { path, position });
        if let Some(previous) = self.debounce.take() {
            previous.remove();
        }
        let weak = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(SAVE_DELAY, move || {
            if let Some(this) = weak.upgrade() {
                // The source is finished once this runs; forget its id rather than remove it.
                this.debounce.replace(None);
                this.send_pending();
            }
        });
        self.debounce.replace(Some(source));
    }

    /// Saves `position` without waiting for the debounce delay.
    pub fn save_now(&self, path: PathBuf, position: ReadingPosition) {
        self.set_pending(SaveRequest { path, position });
        self.flush();
    }

    /// Sends any pending position to the worker immediately.
    pub fn flush(&self) {
        if let Some(source) = self.debounce.take() {
            source.remove();
        }
        self.send_pending();
    }

    /// Flushes pending saves and waits for the worker to write them.
    /// Call once, after the GTK main loop has finished.
    pub fn shutdown(&self) {
        self.flush();
        drop(self.saves.take());
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                glib::g_warning!(LOG_DOMAIN, "the save thread panicked");
            }
        }
    }

    /// Replaces the pending save. A pending save for a different book is sent
    /// first, so switching books never drops the previous book's position.
    fn set_pending(&self, request: SaveRequest) {
        let previous = self.pending.replace(Some(request));
        let current_path = self.pending.borrow().as_ref().map(|r| r.path.clone());
        if let Some(previous) = previous.filter(|p| Some(&p.path) != current_path.as_ref()) {
            self.send(previous);
        }
    }

    fn send_pending(&self) {
        if let Some(request) = self.pending.take() {
            self.send(request);
        }
    }

    fn send(&self, request: SaveRequest) {
        let sent = self
            .saves
            .borrow()
            .as_ref()
            .is_some_and(|saves| saves.send(request).is_ok());
        if !sent && self.store.is_some() {
            glib::g_warning!(
                LOG_DOMAIN,
                "the save thread is not running; position not saved"
            );
        }
    }
}

fn save_blocking(store: &SharedStore, request: &SaveRequest) {
    let result = match store.lock() {
        Ok(store) => store
            .save(&request.path, request.position)
            .map_err(|err| err.to_string()),
        Err(_) => Err("the store lock is poisoned".to_owned()),
    };
    if let Err(err) = result {
        glib::g_warning!(
            LOG_DOMAIN,
            "cannot save reading position for {}: {err}",
            request.path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(chapter: usize, char_offset: usize) -> ReadingPosition {
        ReadingPosition {
            chapter,
            char_offset,
        }
    }

    fn stored(sync: &ProgressSync, path: &str) -> Option<ReadingPosition> {
        let store = sync.store.as_ref().unwrap().lock().unwrap();
        store.load(&PathBuf::from(path)).unwrap()
    }

    #[test]
    fn last_save_wins_after_shutdown() {
        let sync = ProgressSync::new(ProgressStore::open_in_memory().unwrap());
        for offset in 0..50 {
            sync.save_now(PathBuf::from("/b/a.txt"), position(1, offset));
        }
        sync.shutdown();
        assert_eq!(stored(&sync, "/b/a.txt"), Some(position(1, 49)));
    }

    #[test]
    fn switching_books_keeps_previous_books_pending_position() {
        let sync = ProgressSync::new(ProgressStore::open_in_memory().unwrap());
        sync.set_pending(SaveRequest {
            path: PathBuf::from("/b/a.txt"),
            position: position(2, 20),
        });
        sync.set_pending(SaveRequest {
            path: PathBuf::from("/b/b.txt"),
            position: position(3, 30),
        });
        sync.shutdown();
        assert_eq!(stored(&sync, "/b/a.txt"), Some(position(2, 20)));
        assert_eq!(stored(&sync, "/b/b.txt"), Some(position(3, 30)));
    }

    #[test]
    fn load_reads_saved_position_off_main_thread() {
        let sync = ProgressSync::new(ProgressStore::open_in_memory().unwrap());
        sync.save_now(PathBuf::from("/b/a.txt"), position(4, 40));
        sync.shutdown();
        let loaded = glib::MainContext::new().block_on(sync.load(PathBuf::from("/b/a.txt")));
        assert_eq!(loaded, Some(position(4, 40)));
    }
}
