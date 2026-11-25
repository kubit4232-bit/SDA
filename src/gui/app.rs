//! Главное окно SDA — IDM-подобный интерфейс.

use crate::download::{category_for_path, filename_from_url, DownloadJob, DownloadManager, JobStatus};
use crate::gui::icons::SdaIcons;
use crate::network::HttpClient;
use crate::DownloadConfig;
use arboard::Clipboard;
use eframe::egui;
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Debug)]
struct DownloadRow {
    id: Uuid,
    file_name: String,
    url: String,
    path: String,
    status: JobStatus,
    total_bytes: u64,
    downloaded_bytes: u64,
    speed_bps: u64,
    progress: f32,
    category: String,
}

pub struct SdaApp {
    manager: DownloadManager,
    config: DownloadConfig,
    runtime: tokio::runtime::Runtime,
    icons: SdaIcons,
    rows: Vec<DownloadRow>,
    new_url: String,
    new_path: String,
    new_segments: usize,
    new_category: String,
    new_user: String,
    new_pass: String,
    new_cookie: String,
    show_add_dialog: bool,
    status_message: Option<String>,
    status_is_error: bool,
    last_update: Instant,
    clipboard: Option<Clipboard>,
    selected_category: String,
    settings_open: bool,
    about_open: bool,
    speed_limit_mbps: f32,
    max_concurrent: usize,
    max_retries: u32,
    watch_clipboard: bool,
    last_clipboard: String,
    last_clipboard_check: Instant,
    sort_col: usize,
    sort_asc: bool,
    status_shown_at: Option<Instant>,
    search: String,
    output_dir_text: String,
    batch_open: bool,
    batch_text: String,
    sched_enabled: bool,
    sched_start: String,
    sched_stop: String,
    last_sched_check: Instant,
    notified_completed: Vec<Uuid>,
}

#[derive(Clone, PartialEq)]
enum Category {
    All,
    Downloading,
    Documents,
    Music,
    Programs,
    Video,
    Incomplete,
    Completed,
    Waiting,
    Errors,
    Queue,
    Other,
}

impl Category {
    fn all() -> Vec<Category> {
        vec![
            Category::All,
            Category::Downloading,
            Category::Documents,
            Category::Music,
            Category::Programs,
            Category::Video,
            Category::Incomplete,
            Category::Completed,
            Category::Waiting,
            Category::Errors,
            Category::Queue,
            Category::Other,
        ]
    }

    fn title(&self) -> &'static str {
        match self {
            Category::All => "Все загрузки",
            Category::Downloading => "Скачивающие",
            Category::Documents => "Документы",
            Category::Music => "Музыка",
            Category::Programs => "Программы",
            Category::Video => "Видео",
            Category::Incomplete => "Незавершённые",
            Category::Completed => "Завершённые",
            Category::Waiting => "Ожидающие",
            Category::Errors => "Ошибки",
            Category::Queue => "Очередь",
            Category::Other => "Другие",
        }
    }

    fn icon_key(&self) -> &'static str {
        match self {
            Category::All => "categories",
            Category::Downloading => "download",
            Category::Documents => "filename",
            Category::Music => "music",
            Category::Programs => "programs",
            Category::Video => "video",
            Category::Incomplete => "tasks",
            Category::Completed => "launch",
            Category::Waiting => "schedule",
            Category::Errors => "stop",
            Category::Queue => "tasks",
            Category::Other => "filename",
        }
    }
}

impl Default for Category {
    fn default() -> Self {
        Category::All
    }
}

impl SdaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let config = DownloadConfig::load();
        let client = HttpClient::with_config(&config.user_agent, config.connect_timeout_secs, config.read_timeout_secs, config.proxy_url.as_deref());
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        let manager = DownloadManager::new(config.clone(), client.clone());
        let clipboard = Clipboard::new().ok();
        let icons = SdaIcons::new(&cc.egui_ctx);
        Self {
            manager, config: config.clone(), runtime, icons, rows: Vec::new(), new_url: String::new(),
            new_path: String::new(), new_segments: config.default_segment_count,
            new_category: "Файлы".to_string(), show_add_dialog: false,
            new_user: String::new(), new_pass: String::new(), new_cookie: String::new(),
            status_message: None, status_is_error: false,
            last_update: Instant::now(), clipboard,
            selected_category: "Все загрузки".to_string(),
            settings_open: false, about_open: false,
            speed_limit_mbps: config.max_speed_bps.map(|v| v as f32 / 1_048_576.0).unwrap_or(0.0),
            max_concurrent: config.max_concurrent_downloads,
            max_retries: config.max_retries,
            watch_clipboard: config.watch_clipboard,
            last_clipboard: String::new(),
            last_clipboard_check: Instant::now(),
            sort_col: 0, sort_asc: true,
            status_shown_at: None,
            notified_completed: Vec::new(),
            search: String::new(),
            output_dir_text: config.output_dir.to_string_lossy().to_string(),
            batch_open: false,
            batch_text: String::new(),
            sched_enabled: config.sched_enabled,
            sched_start: config.sched_start.clone(),
            sched_stop: config.sched_stop.clone(),
            last_sched_check: Instant::now(),
        }
    }

    // Ширины колонок таблицы (в логических пикселях)
    const COL_STATUS: f32 = 90.0;
    const COL_FILENAME: f32 = 180.0;
    const COL_SOURCE: f32 = 210.0;
    const COL_PROGRESS: f32 = 120.0;
    const COL_SIZE: f32 = 100.0;
    const COL_SPEED: f32 = 80.0;
    const COL_TIME: f32 = 70.0;
    const COL_ACTIONS: f32 = 100.0;
    const ROW_HEIGHT: f32 = 24.0;

    fn show_icon(&self, ui: &mut egui::Ui, name: &str, size: f32) -> Option<egui::Response> {
        let handle = self.icons.get(name)?.clone();
        let image = egui::Image::new((handle.id(), handle.size_vec2()))
            .max_width(size)
            .max_height(size);
        Some(ui.add(image))
    }

    fn status_icon_key(status: &JobStatus) -> &'static str {
        match status {
            JobStatus::Downloading => "download",
            JobStatus::Paused => "pause",
            JobStatus::Queued => "schedule",
            JobStatus::Completed => "launch",
            JobStatus::Failed { .. } => "stop",
            JobStatus::Cancelled => "stop_all",
        }
    }

    fn eta_text(row: &DownloadRow) -> String {
        if row.total_bytes > 0 && row.downloaded_bytes < row.total_bytes && row.speed_bps > 0 {
            let remaining = (row.total_bytes - row.downloaded_bytes) as f64 / row.speed_bps as f64;
            let total_secs = remaining.ceil() as u64;
            let h = total_secs / 3600;
            let m = (total_secs % 3600) / 60;
            let s = total_secs % 60;
            format!("{}:{:02}:{:02}", h, m, s)
        } else {
            "—".to_string()
        }
    }

    fn row_action_for(status: &JobStatus) -> Option<RowAction> {
        match status {
            JobStatus::Downloading => Some(RowAction::Pause),
            JobStatus::Paused => Some(RowAction::Resume),
            JobStatus::Queued => Some(RowAction::Pause),
            _ => None,
        }
    }

    fn draw_download_row(&self, ui: &mut egui::Ui, row: &DownloadRow, pending_actions: &mut Vec<(Uuid, RowAction)>) {
        ui.horizontal(|ui| {
            ui.allocate_ui(egui::vec2(Self::COL_STATUS, Self::ROW_HEIGHT), |ui| {
                let resp = self.show_icon(ui, Self::status_icon_key(&row.status), 14.0);
                if let Some(resp) = resp {
                    if let Some(action) = Self::row_action_for(&row.status) {
                        if resp.clicked() { pending_actions.push((row.id, action)); }
                    }
                }
                ui.colored_label(status_color(&row.status), Self::status_text(&row.status).to_string());
            });            ui.allocate_ui(egui::vec2(Self::COL_FILENAME, Self::ROW_HEIGHT), |ui| { ui.label(&row.file_name); });
            ui.allocate_ui(egui::vec2(Self::COL_SOURCE, Self::ROW_HEIGHT), |ui| {
                let url_display = if row.url.len() > 60 { format!("{}...", &row.url[..57]) } else { row.url.clone() };
                ui.label(url_display);
            });
            ui.allocate_ui(egui::vec2(Self::COL_PROGRESS, Self::ROW_HEIGHT), |ui| {
                ui.add(egui::ProgressBar::new(row.progress).desired_width(Self::COL_PROGRESS).text(format!("{:.1}%", row.progress * 100.0)));
            });
            ui.allocate_ui(egui::vec2(Self::COL_SIZE, Self::ROW_HEIGHT), |ui| {
                let size_text = if row.total_bytes > 0 {
                    format!("{} / {}", Self::format_bytes(row.downloaded_bytes), Self::format_bytes(row.total_bytes))
                } else { Self::format_bytes(row.downloaded_bytes) };
                ui.label(size_text);
            });
            ui.allocate_ui(egui::vec2(Self::COL_SPEED, Self::ROW_HEIGHT), |ui| { ui.label(Self::format_speed(row.speed_bps)); });
            ui.allocate_ui(egui::vec2(Self::COL_TIME, Self::ROW_HEIGHT), |ui| { ui.label(Self::eta_text(row)); });

            ui.allocate_ui(egui::vec2(Self::COL_ACTIONS, Self::ROW_HEIGHT), |ui| {
                ui.horizontal(|ui| {
                    match &row.status {
                        JobStatus::Downloading | JobStatus::Paused | JobStatus::Queued => {
                            let (action, icon) = match &row.status {
                                JobStatus::Downloading | JobStatus::Queued => (RowAction::Pause, "pause"),
                                _ => (RowAction::Resume, "start"),
                            };
                            if let Some(resp) = self.show_icon(ui, icon, 16.0) {
                                if resp.clicked() { pending_actions.push((row.id, action)); }
                            }
                            if let Some(resp) = self.show_icon(ui, "stop_all", 16.0) {
                                if resp.clicked() { pending_actions.push((row.id, RowAction::Cancel)); }
                            }
                        }
                        JobStatus::Completed => {
                            if let Some(resp) = self.show_icon(ui, "launch", 16.0) {
                                if resp.clicked() { pending_actions.push((row.id, RowAction::Open)); }
                            }
                            if let Some(resp) = self.show_icon(ui, "stop_all", 16.0) {
                                if resp.clicked() { pending_actions.push((row.id, RowAction::Remove)); }
                            }
                        }
                        _ => {
                            if let Some(resp) = self.show_icon(ui, "stop_all", 16.0) {
                                if resp.clicked() { pending_actions.push((row.id, RowAction::Remove)); }
                            }
                        }
                    }
                });
            });
        });
    }

    fn refresh_rows(&mut self) {
        let jobs = self.runtime.block_on(self.manager.list_jobs());
        let mut notified_completed = std::mem::take(&mut self.notified_completed);
        for job in jobs.into_iter() {
            if matches!(job.status, JobStatus::Completed)
                && !notified_completed.contains(&job.id)
            {
                notified_completed.push(job.id);
                let name = job
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| job.url.clone());
                self.show_status(format!("Задача завершена: {}", name), false);
            }
            self.rows.push(DownloadRow {
                id: job.id,
                file_name: job.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| job.url.split('/').last().unwrap_or("").to_string()),
                url: job.url.clone(),
                path: job.path.to_string_lossy().to_string(),
                status: job.status.clone(),
                total_bytes: job.total_bytes,
                downloaded_bytes: job.downloaded_bytes,
                speed_bps: job.speed_bps,
                progress: if job.total_bytes > 0 { job.downloaded_bytes as f32 / job.total_bytes as f32 } else { 0.0 },
                category: category_for_path(&job.path).to_string(),
            });
        }
        self.notified_completed = notified_completed;
        let cat = self.selected_category.clone();
        let search = self.search.to_lowercase();
        self.rows.retain(|row| {
            if !search.is_empty()
                && !row.file_name.to_lowercase().contains(&search)
                && !row.url.to_lowercase().contains(&search)
            {
                return false;
            }
            match cat.as_str() {
            "Все загрузки" => true,
            "Скачивающие" => matches!(row.status, JobStatus::Downloading),
            "Завершённые" => matches!(row.status, JobStatus::Completed),
            "Ошибки" => matches!(row.status, JobStatus::Failed { .. }),
            "Ожидающие" => matches!(row.status, JobStatus::Queued),
            "Незавершённые" => matches!(row.status, JobStatus::Paused),
            "Очередь" => matches!(row.status, JobStatus::Queued),
            "Документы" | "Музыка" | "Программы" | "Видео" | "Другие" => row.category == cat,
            _ => true,
            }
        });
        self.rows.sort_by(|a, b| {
            let a_active = matches!(a.status, JobStatus::Downloading);
            let b_active = matches!(b.status, JobStatus::Downloading);
            b_active.cmp(&a_active)
        });
        self.apply_sort();
    }

    fn eta_secs(row: &DownloadRow) -> u64 {
        if row.total_bytes > 0 && row.downloaded_bytes < row.total_bytes && row.speed_bps > 0 {
            ((row.total_bytes - row.downloaded_bytes) as f64 / row.speed_bps as f64).ceil() as u64
        } else {
            u64::MAX
        }
    }

    fn apply_sort(&mut self) {
        let col = self.sort_col;
        let asc = self.sort_asc;
        self.rows.sort_by(|a, b| {
            let ord = match col {
                0 => status_rank(&a.status).cmp(&status_rank(&b.status)),
                1 => a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()),
                2 => a.url.to_lowercase().cmp(&b.url.to_lowercase()),
                3 => a.progress.partial_cmp(&b.progress).unwrap_or(std::cmp::Ordering::Equal),
                4 => a.total_bytes.cmp(&b.total_bytes),
                5 => a.speed_bps.cmp(&b.speed_bps),
                6 => Self::eta_secs(a).cmp(&Self::eta_secs(b)),
                _ => std::cmp::Ordering::Equal,
            };
            if asc { ord } else { ord.reverse() }
        });
    }

    fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["Б", "КБ", "МБ", "ГБ", "ТБ"];
        if bytes == 0 { return "0 Б".to_string(); }
        let mut size = bytes as f64;
        let mut unit_idx = 0;
        while size >= 1024.0 && unit_idx < UNITS.len() - 1 { size /= 1024.0; unit_idx += 1; }
        format!("{:.2} {}", size, UNITS[unit_idx])
    }

    fn format_speed(bps: u64) -> String {
        if bps == 0 { return "—".to_string(); }
        let size = bps as f64;
        if size >= 1_048_576.0 { format!("{:.2} МБ/с", size / 1_048_576.0) }
        else if size >= 1024.0 { format!("{:.2} КБ/с", size / 1024.0) }
        else { format!("{:.0} Б/с", size) }
    }

    fn status_text(status: &JobStatus) -> &'static str {
        match status {
            JobStatus::Queued => "В очереди",
            JobStatus::Downloading => "Скачивание",
            JobStatus::Paused => "Пауза",
            JobStatus::Completed => "Завершено",
            JobStatus::Failed { .. } => "Ошибка",
            JobStatus::Cancelled => "Отменено",
        }
    }

    fn show_status(&mut self, text: String, is_error: bool) {
        self.status_message = Some(text);
        self.status_is_error = is_error;
        self.status_shown_at = Some(Instant::now());
    }

    fn paste_from_clipboard(&mut self) {
        if let Some(clipboard) = &mut self.clipboard {
            match clipboard.get_text() {
                Ok(text) => {
                    let trimmed = text.trim();
                    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                        self.new_url = trimmed.to_string();
                        self.show_status("Ссылка вставлена из буфера обмена".to_string(), false);
                    } else {
                        self.show_status("В буфере обмена нет ссылки".to_string(), true);
                    }
                }
                Err(e) => self.show_status(format!("Ошибка чтения буфера: {}", e), true),
            }
        } else {
            self.show_status("Буфер обмена недоступен".to_string(), true);
        }
    }

    fn apply_schedule(&mut self) {
        let check_interval = std::time::Duration::from_secs(2);
        if self.last_sched_check.elapsed() < check_interval {
            return;
        }
        self.last_sched_check = Instant::now();
        if !self.sched_enabled {
            return;
        }
        let Some(start) = crate::download::state::parse_hms(&self.sched_start) else {
            return;
        };
        let Some(stop) = crate::download::state::parse_hms(&self.sched_stop) else {
            return;
        };
        let now = {
            use chrono::Timelike;
            chrono::Local::now().naive_local().num_seconds_from_midnight() as u64
        };
        let running = self.rows.iter().any(|r| matches!(r.status, JobStatus::Downloading));
        let paused = self.rows.iter().any(|r| matches!(r.status, JobStatus::Paused));
        let before = now < start;
        let after = now > stop;

        if before || after {
            // Перед началом окна или после окончания — все загрузки должны быть на паузе.
            if running {
                let _ = self.runtime.block_on(self.manager.pause_all());
                self.refresh_rows();
            }
        } else if paused {
            // Внутри окна — возобновляем задачи, которые стояли на паузе.
            let _ = self.runtime.block_on(self.manager.resume_all());
            self.refresh_rows();
        }
    }

    fn add_download(&mut self) {
        let url = self.new_url.trim().to_string();
        if url.is_empty() { self.show_status("Введите URL".to_string(), true); return; }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            self.show_status("URL должен начинаться с http:// или https://".to_string(), true);
            return;
        }
        let path = if self.new_path.trim().is_empty() {
            self.config.output_dir.join(filename_from_url(&url))
        } else { PathBuf::from(self.new_path.trim()) };
        let job = DownloadJob {
            id: Uuid::new_v4(), url, save_path: path,
            segment_count: self.new_segments.max(1),
            max_speed_bps: self.config.max_speed_bps,
            username: if self.new_user.trim().is_empty() { None } else { Some(self.new_user.trim().to_string()) },
            password: if self.new_pass.trim().is_empty() { None } else { Some(self.new_pass.trim().to_string()) },
            cookies: if self.new_cookie.trim().is_empty() { None } else { Some(self.new_cookie.trim().to_string()) },
        };
        match self.runtime.block_on(self.manager.add_job(job)) {
            Ok(id) => {
                self.show_status(format!("Задача {} добавлена", id), false);
                self.new_url.clear(); self.new_path.clear();
                self.show_add_dialog = false; self.refresh_rows();
            }
            Err(e) => self.show_status(format!("Ошибка: {}", e), true),
        }
    }

    fn add_batch(&mut self) {
        let urls: Vec<String> = self
            .batch_text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| l.starts_with("http://") || l.starts_with("https://"))
            .collect();
        if urls.is_empty() {
            self.show_status("Нет корректных URL (по одному в строке)".to_string(), true);
            return;
        }
        let mut added = 0usize;
        for url in urls {
            let path = if self.new_path.trim().is_empty() {
                self.config.output_dir.join(filename_from_url(&url))
            } else {
                PathBuf::from(self.new_path.trim())
            };
            let job = DownloadJob {
                id: Uuid::new_v4(),
                url,
                save_path: path,
                segment_count: self.new_segments.max(1),
                max_speed_bps: self.config.max_speed_bps,
                username: if self.new_user.trim().is_empty() { None } else { Some(self.new_user.trim().to_string()) },
                password: if self.new_pass.trim().is_empty() { None } else { Some(self.new_pass.trim().to_string()) },
                cookies: if self.new_cookie.trim().is_empty() { None } else { Some(self.new_cookie.trim().to_string()) },
            };
            if self.runtime.block_on(self.manager.add_job(job)).is_ok() {
                added += 1;
            }
        }
        self.show_status(format!("Добавлено задач: {}", added), added == 0);
        self.batch_open = false;
        self.batch_text.clear();
        self.refresh_rows();
    }

    fn action_on_row(&mut self, id: Uuid, action: RowAction) {
        match action {
            RowAction::Open => {
                if let Some(row) = self.rows.iter().find(|r| r.id == id) {
                    let _ = std::process::Command::new("explorer").arg(&row.path).spawn();
                }
                return;
            }
            RowAction::Remove => {
                let _ = self.runtime.block_on(self.manager.remove_job(id));
                self.refresh_rows();
                return;
            }
            _ => {}
        }
        let result = match action {
            RowAction::Pause => self.runtime.block_on(self.manager.pause_job(id)),
            RowAction::Resume => self.runtime.block_on(self.manager.resume_job(id)),
            RowAction::Cancel => self.runtime.block_on(self.manager.cancel_job(id)),
            _ => Ok(()),
        };
        if let Err(e) = result { self.show_status(format!("Ошибка: {}", e), true); }
        else { self.refresh_rows(); }
    }
}

enum RowAction { Pause, Resume, Cancel, Open, Remove }

impl eframe::App for SdaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_update.elapsed().as_millis() > 100 {
            self.refresh_rows();
            self.last_update = Instant::now();
        }

        // Автоскрытие уведомления
        if self
            .status_shown_at
            .is_some_and(|t| t.elapsed() > std::time::Duration::from_secs(5))
        {
            self.status_message = None;
            self.status_shown_at = None;
        }

        // Синхронизация планировщика (IDM Scheduler)
        self.apply_schedule();

        // IDM-подобный монитор буфера обмена: подставляет новую ссылку в поле URL
        if self.watch_clipboard && self.last_clipboard_check.elapsed().as_millis() > 1200 {
            self.last_clipboard_check = Instant::now();
            if let Some(clipboard) = &mut self.clipboard {
                if let Ok(text) = clipboard.get_text() {
                    let text = text.trim().to_string();
                    if text != self.last_clipboard
                        && (text.starts_with("http://") || text.starts_with("https://"))
                    {
                        self.last_clipboard = text.clone();
                        if self.new_url.trim().is_empty() {
                            self.new_url = text;
                            self.show_status("Ссылка из буфера обмена подставлена".to_string(), false);
                        }
                    }
                }
            }
        }

        if let Some(msg) = &self.status_message {
            let color = if self.status_is_error { egui::Color32::from_rgb(200, 50, 50) } else { egui::Color32::from_rgb(50, 150, 50) };
            egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::RIGHT_BOTTOM, (-10.0, -10.0)).show(ctx, |ui| {
                ui.colored_label(color, msg);
            });
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Задачи", |ui| {
                    if ui.button("Новая загрузка").clicked() { self.show_add_dialog = true; ui.close_menu(); }
                    ui.separator();
                    if ui.button("Выход").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                });
                ui.menu_button("Скачивание", |ui| {
                    if ui.button("Начать загрузку").clicked() { self.show_add_dialog = true; ui.close_menu(); }
                    if ui.button("Пакетная загрузка…").clicked() { self.batch_open = true; ui.close_menu(); }
                    if ui.button("Поставить на паузу").clicked() {
                        for row in &self.rows { if matches!(row.status, JobStatus::Downloading) { let _ = self.runtime.block_on(self.manager.pause_job(row.id)); } }
                        self.refresh_rows(); ui.close_menu();
                    }
                    if ui.button("Возобновить").clicked() {
                        for row in &self.rows { if matches!(row.status, JobStatus::Paused) { let _ = self.runtime.block_on(self.manager.resume_job(row.id)); } }
                        self.refresh_rows(); ui.close_menu();
                    }
                    if ui.button("Остановить всё").clicked() {
                        let _ = self.runtime.block_on(self.manager.stop_all());
                        self.refresh_rows(); ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Удалить завершённые из списка").clicked() {
                        let completed: Vec<Uuid> = self.rows.iter()
                            .filter(|r| matches!(r.status, JobStatus::Completed))
                            .map(|r| r.id).collect();
                        for id in completed { let _ = self.runtime.block_on(self.manager.remove_job(id)); }
                        self.refresh_rows(); ui.close_menu();
                    }
                    if ui.button("Удалить все из списка").clicked() {
                        for row in &self.rows { let _ = self.runtime.block_on(self.manager.remove_job(row.id)); }
                        self.refresh_rows(); ui.close_menu();
                    }
                });
                ui.menu_button("Сервис", |ui| {
                    if ui.button("Настройки").clicked() { self.settings_open = true; ui.close_menu(); }
                    if ui.checkbox(&mut self.watch_clipboard, "Следить за буфером обмена").clicked() { ui.close_menu(); }
                    if ui.button("Плагины").clicked() { self.show_status("Плагины: в разработке".to_string(), false); ui.close_menu(); }
                });
                ui.menu_button("Справка", |ui| {
                    if ui.button("О программе").clicked() { self.about_open = true; ui.close_menu(); }
                });
            });
        });

        egui::SidePanel::left("left_panel").default_width(180.0).show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Категории");
                ui.separator();
                for cat in Category::all() {
                    let selected = self.selected_category == cat.title();
                    ui.horizontal(|ui| {
                        if let Some(resp) = self.show_icon(ui, cat.icon_key(), 16.0) {
                            if resp.clicked() {
                                self.selected_category = cat.title().to_string();
                                self.refresh_rows();
                            }
                        }
                        if ui.selectable_label(selected, cat.title()).clicked() {
                            self.selected_category = cat.title().to_string();
                            self.refresh_rows();
                        }
                    });
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.separator();
                    ui.label("Разработано на Rust");
                    ui.label("REA-реверс инжениринг IDM");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::none().fill(egui::Color32::from_rgb(230, 230, 230)).inner_margin(6.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing();
                    if self.show_icon(ui, "download", 18.0).is_some_and(|r| r.clicked()) { self.show_add_dialog = true; }
                    ui.separator();
                    if self.show_icon(ui, "pause", 18.0).is_some_and(|r| r.clicked()) {
                        let _ = self.runtime.block_on(self.manager.pause_all());
                        self.refresh_rows();
                    }
                    if self.show_icon(ui, "start", 18.0).is_some_and(|r| r.clicked()) {
                        let _ = self.runtime.block_on(self.manager.resume_all());
                        self.refresh_rows();
                    }
                    if self.show_icon(ui, "stop_all", 18.0).is_some_and(|r| r.clicked()) {
                        let _ = self.runtime.block_on(self.manager.stop_all());
                        self.refresh_rows();
                    }
                    ui.separator();
                    if self.show_icon(ui, "settings", 18.0).is_some_and(|r| r.clicked()) { self.settings_open = true; }
                    let limit_text = if self.speed_limit_mbps > 0.0 {
                        format!("Лимит: {:.1} МБ/с", self.speed_limit_mbps)
                    } else {
                        "Лимит: выкл".to_string()
                    };
                    ui.label(limit_text);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Всего: {}", self.rows.len()));
                    });
                });
            });

            ui.separator();

            egui::Frame::none().fill(egui::Color32::from_rgb(240, 240, 240)).inner_margin(8.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("URL:");
                    ui.text_edit_singleline(&mut self.new_url);
                    if self.show_icon(ui, "clipboard", 16.0).is_some_and(|r| r.clicked()) { self.paste_from_clipboard(); }
                    if self.show_icon(ui, "download", 16.0).is_some_and(|r| r.clicked()) { self.add_download(); }
                    ui.label("Сегментов:");
                    ui.add(egui::DragValue::new(&mut self.new_segments).clamp_range(1..=32));
                    ui.separator();
                    ui.label("Поиск:");
                    ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(140.0));
                });
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let sc = self.sort_col;
                    let sa = self.sort_asc;
                    let mut clicked: Option<usize> = None;
                    if header_cell(ui, "Статус", Self::COL_STATUS, 0, sc, sa) { clicked = Some(0); }
                    if header_cell(ui, "Имя файла", Self::COL_FILENAME, 1, sc, sa) { clicked = Some(1); }
                    if header_cell(ui, "Источник", Self::COL_SOURCE, 2, sc, sa) { clicked = Some(2); }
                    if header_cell(ui, "Прогресс", Self::COL_PROGRESS, 3, sc, sa) { clicked = Some(3); }
                    if header_cell(ui, "Размер", Self::COL_SIZE, 4, sc, sa) { clicked = Some(4); }
                    if header_cell(ui, "Скорость", Self::COL_SPEED, 5, sc, sa) { clicked = Some(5); }
                    if header_cell(ui, "Осталось", Self::COL_TIME, 6, sc, sa) { clicked = Some(6); }
                    ui.allocate_ui(egui::vec2(Self::COL_ACTIONS, 22.0), |ui| { ui.label("Действия"); });
                    if let Some(col) = clicked {
                        self.sort_asc = if self.sort_col == col { !self.sort_asc } else { true };
                        self.sort_col = col;
                        self.apply_sort();
                    }
                });
                ui.separator();

                let mut pending_actions: Vec<(Uuid, RowAction)> = Vec::new();

                for row in &self.rows {
                    self.draw_download_row(ui, row, &mut pending_actions);
                }

                for (action_id, action) in pending_actions {
                    self.action_on_row(action_id, action);
                }
            });

            if self.show_add_dialog {
                egui::Window::new("Новая загрузка").collapsible(false).resizable(false).show(ctx, |ui| {
                    ui.set_width(400.0);
                    ui.label("URL:");
                    ui.text_edit_singleline(&mut self.new_url);
                    ui.label("Путь сохранения (пусто — папка загрузок):");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.new_path);
                        if ui.button("Обзор…").clicked() { self.show_status("Файловый диалог: в разработке".to_string(), false); }
                    });
                    ui.label(format!("Итоговый файл: {}", if self.new_url.trim().is_empty() {
                        "—".to_string()
                    } else if self.new_path.trim().is_empty() {
                        self.config.output_dir.join(filename_from_url(self.new_url.trim())).display().to_string()
                    } else {
                        self.new_path.clone()
                    }));
                    ui.label("Категория:");
                    ui.label(&self.new_category);
                    ui.label("Логин (пароль для сайта):");
                    ui.text_edit_singleline(&mut self.new_user);
                    ui.label("Пароль (пароль для сайта):");
                    ui.text_edit_singleline(&mut self.new_pass);
                    ui.label("Cookie (строка Cookie для сайта):");
                    ui.text_edit_singleline(&mut self.new_cookie);
                    ui.label("Сегментов:");
                    ui.add(egui::DragValue::new(&mut self.new_segments).clamp_range(1..=32));
                    ui.horizontal(|ui| {
                        if self.show_icon(ui, "start", 16.0).is_some_and(|r| r.clicked()) { self.add_download(); }
                        if self.show_icon(ui, "stop_all", 16.0).is_some_and(|r| r.clicked()) { self.show_add_dialog = false; }
                    });
                });
            }
            if self.batch_open {
                egui::Window::new("Пакетная загрузка")
                    .collapsible(false)
                    .resizable(true)
                    .show(ctx, |ui| {
                        ui.set_width(500.0);
                        ui.label("По одной ссылке в строке (http/https):");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.batch_text)
                                .desired_rows(8)
                                .desired_width(480.0),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Добавить все").clicked() { self.add_batch(); }
                            if ui.button("Отмена").clicked() { self.batch_open = false; }
                        });
                    });
            }

            if self.settings_open {
                let mut open = true;
                egui::Window::new("Настройки").open(&mut open).resizable(false).show(ctx, |ui| {
                    ui.set_width(420.0);
                    ui.heading("Загрузка");
                    ui.horizontal(|ui| {
                        ui.label("Сегментов на файл:");
                        ui.add(egui::DragValue::new(&mut self.new_segments).clamp_range(1..=32));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Одновременных загрузок:");
                        ui.add(egui::DragValue::new(&mut self.max_concurrent).clamp_range(1..=16));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Повторов при ошибке:");
                        ui.add(egui::DragValue::new(&mut self.max_retries).clamp_range(0..=10));
                    });
                    ui.separator();
                    ui.heading("Speed Limiter");
                    ui.horizontal(|ui| {
                        ui.label("Лимит скорости:");
                        ui.add(egui::DragValue::new(&mut self.speed_limit_mbps)
                            .speed(0.1).clamp_range(0.0..=1000.0)
                            .suffix(" МБ/с"));
                    });
                    ui.label("0 — без ограничения (как в IDM: выключен)");
                    ui.separator();
                    ui.heading("Планировщик (Scheduler)");
                    ui.checkbox(&mut self.sched_enabled, "Автозапуск/автостоп задач по расписанию");
                    ui.label("Запуск задач: ");
                    ui.add(egui::TextEdit::singleline(&mut self.sched_start).desired_width(90.0));
                    ui.label("Остановка задач: ");
                    ui.add(egui::TextEdit::singleline(&mut self.sched_stop).desired_width(90.0));
                    ui.separator();
                    ui.heading("Параметры");
                    ui.checkbox(&mut self.watch_clipboard, "Следить за буфером обмена (IDM-подобный перехват ссылок)");
                    ui.horizontal(|ui| {
                        ui.label("Папка загрузок:");
                        ui.add(egui::TextEdit::singleline(&mut self.output_dir_text).desired_width(300.0));
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Применить").clicked() {
                            let mut cfg = self.config.clone();
                            cfg.default_segment_count = self.new_segments;
                            cfg.max_concurrent_downloads = self.max_concurrent;
                            cfg.max_retries = self.max_retries;
                            cfg.watch_clipboard = self.watch_clipboard;
                            cfg.sched_enabled = self.sched_enabled;
                            cfg.sched_start = self.sched_start.clone();
                            cfg.sched_stop = self.sched_stop.clone();
                            if !self.output_dir_text.trim().is_empty() {
                                cfg.output_dir = PathBuf::from(self.output_dir_text.trim());
                            }
                            cfg.max_speed_bps = if self.speed_limit_mbps > 0.0 {
                                Some((self.speed_limit_mbps as f64 * 1_048_576.0) as u64)
                            } else {
                                None
                            };
                            cfg.save();
                            self.config = cfg.clone();
                            self.runtime.block_on(self.manager.update_config(cfg));
                            self.show_status("Настройки применены и сохранены".to_string(), false);
                            self.settings_open = false;
                        }
                        if ui.button("Закрыть").clicked() { self.settings_open = false; }
                    });
                });
                if !open { self.settings_open = false; }
            }

            if self.about_open {
                let mut open = true;
                egui::Window::new("О программе").open(&mut open).resizable(false).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("SDA — Super Download Agent");
                    ui.label("Версия 0.1.0 (клон Internet Download Manager)");
                    ui.label("Движок: Rust + Tokio, сегментированная загрузка (Range).");
                    ui.label("Интерфейс: egui, схема и колонки по данным REA-анализа IDM.");
                    ui.label("Поддержка: HTTP/HTTPS, YouTube через yt-dlp, Named Pipe IPC.");
                    ui.separator();
                    ui.hyperlink_to("github.com/kubit4232-bit/SDA", "https://github.com/kubit4232-bit/SDA");
                });
                if !open { self.about_open = false; }
            }
        });

        let active = self.rows.iter().filter(|r| matches!(r.status, JobStatus::Downloading)).count();
        let total_speed: u64 = self.rows.iter()
            .filter(|r| matches!(r.status, JobStatus::Downloading))
            .map(|r| r.speed_bps)
            .sum();
        let completed = self.rows.iter().filter(|r| matches!(r.status, JobStatus::Completed)).count();

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(if self.status_message.is_some() { "Сообщение" } else { "Готово" });
                ui.separator();
                ui.label(format!("Активных: {}", active));
                ui.separator();
                ui.label(format!("Завершено: {}", completed));
                ui.separator();
                ui.label(format!("Скорость: {}", Self::format_speed(total_speed)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("v0.1.0");
                    ui.separator();
                    ui.label("SDA — Super Download Agent");
                });
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn status_rank(status: &JobStatus) -> u8 {
    match status {
        JobStatus::Downloading => 0,
        JobStatus::Queued => 1,
        JobStatus::Paused => 2,
        JobStatus::Failed { .. } => 3,
        JobStatus::Completed => 4,
        JobStatus::Cancelled => 5,
    }
}

/// Кликабельный заголовок колонки с индикатором сортировки.
fn header_cell(
    ui: &mut egui::Ui,
    title: &str,
    width: f32,
    col: usize,
    current_col: usize,
    asc: bool,
) -> bool {
    let mut clicked = false;
    ui.allocate_ui(egui::vec2(width, 22.0), |ui| {
        let label = if current_col == col {
            format!("{} {}", title, if asc { "▲" } else { "▼" })
        } else {
            title.to_string()
        };
        if ui.add(egui::Button::new(label).frame(false)).clicked() {
            clicked = true;
        }
    });
    clicked
}

fn status_color(status: &JobStatus) -> egui::Color32 {
    match status {
        JobStatus::Downloading => egui::Color32::from_rgb(0, 150, 0),
        JobStatus::Completed => egui::Color32::from_rgb(100, 200, 100),
        JobStatus::Paused => egui::Color32::from_rgb(255, 150, 0),
        JobStatus::Failed { .. } => egui::Color32::from_rgb(255, 80, 80),
        JobStatus::Cancelled => egui::Color32::from_rgb(150, 150, 150),
        JobStatus::Queued => egui::Color32::from_rgb(200, 200, 200),
    }
}
