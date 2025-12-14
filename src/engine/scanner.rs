use ignore::WalkBuilder;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::sync::mpsc::Sender;

pub enum Message {
    FileFound(PathBuf, u64), // Path, Size
    Finished,
    Error(String),
}

pub struct Scanner {
    tx: Sender<Message>,
    stop_signal: Arc<AtomicBool>,
}

impl Scanner {
    pub fn new(tx: Sender<Message>) -> Self {
        Self {
            tx,
            stop_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }

    pub fn start(&self, root: PathBuf) {
        let tx = self.tx.clone();
        let stop = self.stop_signal.clone();

        thread::spawn(move || {
            let walker = WalkBuilder::new(root)
                .standard_filters(false) // Don't implicitly ignore gitignore unless desired, but usually for backup tools we might want to be explicit. Let's keep it simple for now and process everything.
                .hidden(false)
                .threads(4) // Parallel walking
                .build_parallel();

            walker.run(|| {
                let tx = tx.clone();
                let stop = stop.clone();
                Box::new(move |entry| {
                    if stop.load(Ordering::Relaxed) {
                        return ignore::WalkState::Quit;
                    }

                    match entry {
                        Ok(dent) => {
                            if dent.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                                let path = dent.path().to_path_buf();
                                let size = dent.metadata().map(|m| m.len()).unwrap_or(0);
                                // Low level message sending
                                let _ = tx.send(Message::FileFound(path, size));
                            }
                        },
                        Err(e) => {
                             let _ = tx.send(Message::Error(e.to_string()));
                        }
                    }
                    ignore::WalkState::Continue
                })
            });

             let _ = tx.send(Message::Finished);
        });
    }
}
