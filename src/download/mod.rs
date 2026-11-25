//! Модуль загрузки: сегментированное скачивание, resume, менеджер задач

pub mod control;
pub mod engine;
pub mod job;
pub mod manager;
pub mod segment;
pub mod state;
pub mod youtube_extractor;

pub use engine::download_file;
pub use control::JobControl;
pub use job::{category_for_path, filename_from_url, DownloadJob, JobInfo, JobStatus};
pub use manager::DownloadManager;
pub use state::{portable_data_dir, CategoryFolder, ConnectionException, DownloadConfig, JobState, SegmentState, SiteLogin, StateManager};
pub use youtube_extractor::{extract_video_info, extract_video_url, is_video_url};
