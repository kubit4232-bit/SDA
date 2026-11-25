//! Конфигурация и состояние загрузчика SDA

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::RwLock;

/// Конфигурация загрузчика SDA
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadConfig {
    /// Количество сегментов по умолчанию на файл
    pub default_segment_count: usize,
    /// Максимальное количество одновременных загрузок
    pub max_concurrent_downloads: usize,
    /// Максимальная скорость загрузки в байт/сек (None = без ограничения)
    pub max_speed_bps: Option<u64>,
    /// Таймаут подключения в секундах
    pub connect_timeout_secs: u64,
    /// Таймаут чтения в секундах
    pub read_timeout_secs: u64,
    /// Максимальное количество попыток при ошибке
    pub max_retries: u32,
    /// Папка для временных файлов сегментов
    pub temp_dir: PathBuf,
    /// Папка для готовых файлов
    pub output_dir: PathBuf,
    /// User-Agent
    pub user_agent: String,
    /// Следить за буфером обмена (IDM-подобный перехват ссылок)
    pub watch_clipboard: bool,
    /// Планировщик (IDM Scheduler): авто-запуск/остановка по расписанию
    pub sched_enabled: bool,
    /// Время старта задач: "HH:MM"
    pub sched_start: String,
    /// Время остановки задач: "HH:MM"
    pub sched_stop: String,
    /// URL прокси для всех запросов (если задан)
    pub proxy_url: Option<String>,

    // ── Вкладка «Общее» (General) ─────────────────────────────────────
    /// Запускать SDA при входе в Windows
    pub start_with_windows: bool,
    /// Расширенная интеграция в браузеры
    pub browser_integration: bool,
    /// Перехват при нажатии Ctrl + клик
    pub capture_ctrl_key: bool,
    /// Запрет перехвата при нажатии Alt + клик
    pub capture_alt_key: bool,
    /// Обнаруживать новые приложения
    pub detect_new_apps: bool,

    // ── Вкладка «Типы Файлов» ─────────────────────────────────────────
    /// Расширения, перехватываемые автоматически
    pub file_types: Vec<String>,
    /// Сайты, с которых не начинать автоматическую загрузку
    pub excluded_sites: Vec<String>,
    /// Исключённые URL
    pub excluded_urls: Vec<String>,

    // ── Вкладка «Соединение» ──────────────────────────────────────────
    /// Тип соединения: high / medium / low
    pub connection_type: String,
    /// Исключения: число соединений для сайта
    pub connection_exceptions: Vec<ConnectionException>,
    /// Ограничения по загрузке (квоты)
    pub download_limits_enabled: bool,
    pub download_limit_mb: u64,
    pub download_limit_hours: u64,

    // ── Вкладка «Сохранение» ──────────────────────────────────────────
    /// Папки по категориям
    pub category_folders: Vec<CategoryFolder>,
    /// Запоминать последний путь для категории
    pub remember_last_path: bool,

    // ── Вкладка «Загрузка» ────────────────────────────────────────────
    /// Показывать диалог прогресса: show / minimize / hide
    pub progress_dialog: String,
    pub show_start_dialog: bool,
    pub show_complete_dialog: bool,
    /// Тихий режим
    pub silent_mode: bool,
    /// Начинать загрузку сразу при показе диалога
    pub start_immediately: bool,
    /// Игнорировать смену даты файла при докачке
    pub ignore_date_on_resume: bool,
    /// Антивирус: путь и параметры
    pub antivirus_path: Option<String>,
    pub antivirus_params: String,

    // ── Вкладка «Прокси» ──────────────────────────────────────────────
    pub proxy_user: Option<String>,
    pub proxy_pass: Option<String>,
    pub ftp_pasv: bool,
    pub use_ie_proxy: bool,
    pub proxy_exclusions: Vec<String>,

    // ── Вкладка «Пароли сайтов» ───────────────────────────────────────
    pub site_logins: Vec<SiteLogin>,

    // ── Вкладка «Dial Up» ─────────────────────────────────────────────
    pub dialup_connection: String,
    pub dialup_user: String,
    pub dialup_pass: String,
    pub dialup_save_password: bool,
    pub dialup_attempts: u32,
    pub dialup_interval_secs: u64,

    // ── Вкладка «Звуки» ───────────────────────────────────────────────
    pub sound_complete: Option<String>,
    pub sound_error: Option<String>,
    pub sound_start: Option<String>,

    // ── Очереди и кастомизация ────────────────────────────────────────
    pub auto_start_next_in_queue: bool,
    pub toolbar_style: u8,
    pub visible_columns: Vec<String>,

    // ── Резерв/Сканирование ───────────────────────────────────────────
    pub backup_enabled: bool,
    pub backup_dir: Option<PathBuf>,
    pub scan_downloads: bool,
}

/// Логин сайта (вкладка «Пароли сайтов»)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteLogin {
    pub url: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub cookie: String,
}

/// Исключение для соединения (число потоков на сайт)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionException {
    pub site: String,
    pub connections: usize,
}

/// Папка категории (вкладка «Сохранение»)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryFolder {
    pub category: String,
    pub path: PathBuf,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {            default_segment_count: 8,
            max_concurrent_downloads: 4,
            max_speed_bps: None,
            connect_timeout_secs: 30,
            read_timeout_secs: 60,
            max_retries: 3,
            temp_dir: std::env::temp_dir().join("sda"),
            output_dir: default_output_dir(),
            user_agent: format!("SDA/{} (Windows x64)", env!("CARGO_PKG_VERSION")),
            watch_clipboard: true,
            sched_enabled: false,
            sched_start: "06:00".to_string(),
            sched_stop: "23:00".to_string(),
            proxy_url: None,
            start_with_windows: false,
            browser_integration: true,
            capture_ctrl_key: true,
            capture_alt_key: true,
            detect_new_apps: false,
            file_types: vec![
                "zip", "rar", "7z", "exe", "msi", "mp3", "mp4", "mkv", "avi", "mov",
                "pdf", "doc", "docx", "xls", "xlsx", "iso", "gz", "tar", "webm", "flac",
            ].into_iter().map(str::to_string).collect(),
            excluded_sites: Vec::new(),
            excluded_urls: Vec::new(),
            connection_type: "high".to_string(),
            connection_exceptions: Vec::new(),
            download_limits_enabled: false,
            download_limit_mb: 0,
            download_limit_hours: 4,
            category_folders: Vec::new(),
            remember_last_path: true,
            progress_dialog: "show".to_string(),
            show_start_dialog: true,
            show_complete_dialog: true,
            silent_mode: false,
            start_immediately: true,
            ignore_date_on_resume: true,
            antivirus_path: None,
            antivirus_params: String::new(),
            proxy_user: None,
            proxy_pass: None,
            ftp_pasv: true,
            use_ie_proxy: false,
            proxy_exclusions: Vec::new(),
            site_logins: Vec::new(),
            dialup_connection: String::new(),
            dialup_user: String::new(),
            dialup_pass: String::new(),
            dialup_save_password: false,
            dialup_attempts: 0,
            dialup_interval_secs: 30,
            sound_complete: None,
            sound_error: None,
            sound_start: None,
            auto_start_next_in_queue: true,
            toolbar_style: 0,
            visible_columns: vec![
                "Имя файла", "Очередь", "Размер", "Состояние", "Осталось времени",
                "Скорость передачи", "Дата последней попытки", "Описание",
            ].into_iter().map(str::to_string).collect(),
            backup_enabled: false,
            backup_dir: None,
            scan_downloads: false,
        }
    }
}

/// Каталог данных. Если рядом с exe есть файл-маркер `portable.txt`,
/// настройки и список задач хранятся в каталоге программы (портативный режим).
pub fn portable_data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("portable.txt").is_file() {
                return dir.to_path_buf();
            }
        }
    }
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("SDA")
}

impl DownloadConfig {
    /// Путь к файлу настроек.
    pub fn config_path() -> PathBuf {
        portable_data_dir().join("settings.json")
    }

    /// Загрузить настройки (при отсутствии/ошибке — значения по умолчанию).
    pub fn load() -> Self {
        std::fs::read_to_string(Self::config_path())
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    /// Сохранить настройки.
    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// Время "HH:MM" как число секунд с начала суток.
pub fn parse_hms(value: &str) -> Option<u64> {
    let parts: Vec<&str> = value.trim().split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let hour: u64 = parts[0].parse().ok()?;
    let minute: u64 = parts[1].parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 3600 + minute * 60)
}

/// Папка загрузок по умолчанию (как в IDM: профиль\Downloads).
fn default_output_dir() -> PathBuf {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let downloads = PathBuf::from(profile).join("Downloads");
        if downloads.is_dir() {
            return downloads;
        }
    }
    std::env::temp_dir().join("sda_downloads")
}

/// Состояние загруженного фрагмента для resume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentState {
    pub segment_index: u32,
    pub start_byte: u64,
    pub end_byte: u64,
    pub downloaded: u64,
    pub completed: bool,
}

/// Состояние задачи на диске (для восстановления после перезапуска)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobState {
    pub id: String,
    pub url: String,
    pub save_path: String,
    pub total_size: Option<u64>,
    pub downloaded_total: u64,
    pub segments: Vec<SegmentState>,
    pub status: String,
}

/// Менеджер сохранённого состояния задач
pub struct StateManager {
    #[allow(dead_code)]
    states: RwLock<Vec<JobState>>,
    state_dir: std::path::PathBuf,
}

impl StateManager {
    pub fn new(state_dir: std::path::PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&state_dir);
        Self {
            states: RwLock::new(Vec::new()),
            state_dir,
        }
    }

    pub async fn save_job_state(&self, state: &JobState) -> anyhow::Result<()> {
        let path = self.state_dir.join(format!("{}.json", state.id));
        let json = serde_json::to_string_pretty(state)?;
        tokio::fs::write(&path, json).await?;
        Ok(())
    }

    pub async fn load_job_state(&self, id: &str) -> Option<JobState> {
        let path = self.state_dir.join(format!("{}.json", id));
        let data = tokio::fs::read_to_string(&path).await.ok()?;
        serde_json::from_str(&data).ok()
    }

    pub async fn delete_job_state(&self, id: &str) -> anyhow::Result<()> {
        let path = self.state_dir.join(format!("{}.json", id));
        let _ = tokio::fs::remove_file(&path).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_marker_switches_data_dir() {
        let exe = std::env::current_exe().expect("current_exe");
        let dir = exe.parent().expect("exe directory");
        let marker = dir.join("portable.txt");
        let normal = portable_data_dir();
        std::fs::write(&marker, b"portable").expect("write marker");
        let portable = portable_data_dir();
        let _ = std::fs::remove_file(&marker);
        assert_eq!(portable, dir);
        assert_ne!(normal, portable);
        assert_eq!(DownloadConfig::config_path(), normal.join("settings.json"));
    }
}
