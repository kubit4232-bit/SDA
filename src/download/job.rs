//! Типы задач загрузки

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Существующая задача загрузки (передаётся в DownloadManager)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: Uuid,
    pub url: String,
    pub save_path: PathBuf,
    pub segment_count: usize,
    pub max_speed_bps: Option<u64>,
    /// Логин для авторизации (Basic Auth, как в «Пароли сайтов» IDM)
    #[serde(default)]
    pub username: Option<String>,
    /// Пароль для авторизации (Basic Auth, как в «Пароли сайтов» IDM)
    #[serde(default)]
    pub password: Option<String>,
    /// Cookie-цепочка для авторизации (как в «Пароли сайтов» / cookie-флоу IDM)
    #[serde(default)]
    pub cookies: Option<String>,
}

/// Информация о задаче для внешних потребителей (IPC, GUI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: Uuid,
    pub url: String,
    pub path: PathBuf,
    pub status: JobStatus,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub segment_count: usize,
    /// Логин (Basic Auth) — как в «Пароли сайтов» IDM
    #[serde(default)]
    pub username: Option<String>,
    /// Пароль (Basic Auth) — как в «Пароли сайтов» IDM
    #[serde(default)]
    pub password: Option<String>,
    /// Cookie-цепочка — как в cookie-флоу IDM
    #[serde(default)]
    pub cookies: Option<String>,
}

/// Статус задачи
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed { message: String },
    Cancelled,
}

/// Имя файла, выводимое из URL (как IDM предлагает по умолчанию).
pub fn filename_from_url(url: &str) -> String {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    let last = without_query
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim();
    let mut name = if last.is_empty() {
        "download".to_string()
    } else {
        // Минимальное URL-декодирование (%20 и т.п.)
        let mut decoded = String::with_capacity(last.len());
        let bytes = last.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%'
                && i + 2 < bytes.len()
                && last.is_char_boundary(i + 1)
                && last.is_char_boundary(i + 3)
            {
                if let Ok(v) = u8::from_str_radix(&last[i + 1..i + 3], 16) {
                    decoded.push(v as char);
                    i += 3;
                    continue;
                }
            }
            decoded.push(bytes[i] as char);
            i += 1;
        }
        decoded
    };
    for c in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
        name = name.replace(c, "_");
    }
    if name.is_empty() {
        name = "download".to_string();
    }
    name
}

/// Категория файла по расширению (как группы IDM).
pub fn category_for_path(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
        | "ts" | "3gp" => "Видео",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "aiff" => "Музыка",
        "exe" | "msi" | "zip" | "rar" | "7z" | "iso" | "apk" | "dmg" | "tar" | "gz" | "bz2"
        | "xz" | "cab" | "deb" | "rpm" => "Программы",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "rtf" | "odt"
        | "csv" | "epub" | "md" | "json" | "xml" => "Документы",
        _ => "Другие",
    }
}
