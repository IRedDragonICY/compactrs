use windows::Win32::Foundation::HWND;
use crossbeam_channel::{unbounded, Receiver, Sender};
use crate::engine::scanner::Message;
use std::sync::{Arc, atomic::AtomicBool};

pub enum UiMessage {
    Progress(u64, u64), // current, total
    Log(String),
    Status(String),
    Finished,
    Error(String),
}

pub struct Controls {
    pub list_view: HWND,
    pub btn_scan: HWND,
    pub btn_compress: HWND,
    pub btn_decompress: HWND,
    pub combo_algo: HWND,
    pub static_text: HWND,
    pub progress_bar: HWND,
    pub btn_cancel: HWND,
}

pub struct AppState {
    pub current_folder: Option<String>,
    pub controls: Option<Controls>,
    pub tx: Sender<UiMessage>,
    pub rx: Receiver<UiMessage>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            current_folder: None,
            controls: None,
            tx,
            rx,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}
