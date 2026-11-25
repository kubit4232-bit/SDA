//! # SDA Core — Super Download Agent
//!
//! Высокопроизводительный многопоточный загрузчик файлов с поддержкой:
//! - сегментирования (Range-запросы),
//! - возобновления загрузки (resume),
//! - IPC через именованные каналы (аналог IDMNetMon / IDMMsgHost).

pub mod download;
pub mod network;
pub mod protocol;

#[cfg(feature = "gui")]
pub mod gui;

#[cfg(feature = "gui")]
pub mod native_gui;

// Ре-экспорт основных типов для удобства
pub use download::state::DownloadConfig;
pub use download::{DownloadManager, DownloadJob, JobInfo, JobStatus};
pub use download::youtube_extractor::{extract_video_info, extract_video_url, is_video_url, VideoInfo};
pub use network::HttpClient;
pub use protocol::SdaMessage;



use anyhow::Result;
use tracing::info;

/// Версия агента
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Точка входа библиотеки — инициализация субсистем
pub fn init() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("SDA Core v{} initialized", VERSION);
    Ok(())
}
