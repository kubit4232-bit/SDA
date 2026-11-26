//! Нативный Win32 GUI в стиле Internet Download Manager (Rust + windows-rs).
//!
//! Использует те же системные контролы, что и IDM:
//!   SysTreeView32 (категории), SysListView32 (список загрузок),
//!   ToolbarWindow32 (панель инструментов), msctls_statusbar32 (статус-бар),
//!   меню через CreateMenu/AppendMenuW.

use crate::download::{filename_from_url, DownloadJob, JobInfo, JobStatus};
use crate::{DownloadConfig, DownloadManager, HttpClient};
use std::collections::HashSet;
use std::ffi::c_void;
use std::path::PathBuf;
use std::time::Instant;
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{COLORREF, FALSE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, HBRUSH, UpdateWindow};
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetAsyncKeyState, GetFocus, SetFocus, VK_CONTROL, VK_MENU,
};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES, ICC_TREEVIEW_CLASSES,
    INITCOMMONCONTROLSEX, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT, LVCF_WIDTH, LVCFMT_LEFT,
    ILC_COLOR32, ILC_MASK, ImageList_AddMasked, ImageList_Create, PBM_SETPOS,
    LVIF_TEXT, LVIF_PARAM, LVCOLUMNW, LVITEMW, LVM_DELETEALLITEMS, LVM_INSERTCOLUMNW,
    HDN_ITEMCLICKW, NMHEADERW,
    LVM_INSERTITEMW, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMW, LVS_EX_FULLROWSELECT,
    LVS_EX_GRIDLINES, LVS_REPORT, LVS_SINGLESEL, NMHDR, NM_DBLCLK, SB_SETPARTS,
    SB_SETTEXTW, TBBUTTON, TBSTATE_ENABLED, TBSTYLE_FLAT, TB_ADDSTRINGW, TB_AUTOSIZE,
    TB_INSERTBUTTONW, TB_SETIMAGELIST, TCIF_TEXT, TCM_GETCURSEL, TCM_INSERTITEMW, TCN_SELCHANGE,
    TCITEMW, TCS_MULTILINE, TVIF_TEXT, TVINSERTSTRUCTW, TVINSERTSTRUCTW_0, TVI_LAST,
    TVI_ROOT, TVITEMW, TVM_GETITEMW, TVM_GETNEXTITEM, TVM_INSERTITEMW, TVS_HASBUTTONS,
    TVS_HASLINES, TVS_LINESATROOT, WC_LISTVIEW, WC_TABCONTROL, WC_TREEVIEW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD,
    NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS,
    BROWSEINFOW, SHBrowseForFolderW, SHGetPathFromIDListW,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
    OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateMenu, CreatePopupMenu, CreateWindowExW, DC_HASDEFID, DefWindowProcW,
    DestroyWindow, DispatchMessageW, DrawMenuBar, GetClientRect, GetCursorPos, GetMessageW,
    GetParent, GetWindow, GetWindowLongPtrW, IsDialogMessageW, KillTimer, LoadCursorW,
    LoadIconW, MessageBoxW, PostQuitMessage, RegisterClassExW, SendMessageW, SetForegroundWindow,
    SetMenu, SetTimer, SetWindowLongPtrW, ShowWindow, TrackPopupMenu, TranslateMessage, WNDCLASS_STYLES,
    WNDCLASSEXW, GWLP_USERDATA, GWL_STYLE, GW_CHILD, GW_HWNDNEXT, IDC_ARROW, IDI_APPLICATION, MF_POPUP, MF_STRING,
    TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_APP, WM_CONTEXTMENU, HICON, LoadImageW,
    IMAGE_ICON, LR_LOADFROMFILE, IMAGE_FLAGS, ES_AUTOHSCROLL, WS_TABSTOP,
    SW_SHOW, WS_BORDER, WS_CHILD, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const ID_TREE: i32 = 1001;
const ID_LIST: i32 = 1002;
const ID_TOOLBAR: i32 = 1003;
const ID_STATUS: i32 = 1004;
const ID_TIMER: usize = 1;

// Меню «Задачи»
const CMD_ADD: usize = 2001;
const CMD_START: usize = 2002;
const CMD_PAUSE: usize = 2003;
const CMD_RESUME: usize = 2004;
const CMD_STOP: usize = 2005;
const CMD_REMOVE: usize = 2006;
const CMD_START_ALL: usize = 2007;
const CMD_STOP_ALL: usize = 2008;
const CMD_EXIT: usize = 2009;
// Прочее
const CMD_SETTINGS: usize = 2020;
const CMD_SCHEDULE: usize = 2021;
const CMD_GRABBER: usize = 2022;
const CMD_ABOUT: usize = 2023;
const CMD_CLEAR: usize = 2024;
const CMD_LAUNCH: usize = 2025;
const CMD_CAT_ALL: usize = 2030;
const CMD_CAT_COMPLETED: usize = 2031;
const CMD_CAT_UNFINISHED: usize = 2032;
const CMD_CAT_QUEUE: usize = 2033;
// Контекстное меню списка
const CMD_CTX_START: usize = 2100;
const CMD_CTX_PAUSE: usize = 2101;
const CMD_CTX_RESUME: usize = 2102;
const CMD_CTX_CANCEL: usize = 2103;
const CMD_CTX_REMOVE: usize = 2104;
const CMD_CTX_PROPERTIES: usize = 2105;
const CMD_CTX_OPEN_FOLDER: usize = 2106;
const CMD_CTX_COPY_URL: usize = 2107;
// Пункты меню IDM 6.32
const CMD_ADD_BATCH: usize = 2200;
const CMD_ADD_BATCH_CLIP: usize = 2201;
const CMD_GRABBER_SITE: usize = 2202;
const CMD_DROP_TARGET: usize = 2203;
const CMD_EXPORT: usize = 2204;
const CMD_IMPORT: usize = 2205;
const CMD_REDOWNLOAD: usize = 2206;
const CMD_MOVE_RENAME: usize = 2207;
const CMD_PAUSE_ALL: usize = 2208;
const CMD_DELETE_COMPLETED: usize = 2209;
const CMD_START_QUEUE: usize = 2210;
const CMD_STOP_QUEUE: usize = 2211;
const CMD_SPEED_LIMIT: usize = 2212;
const CMD_FIND: usize = 2213;
const CMD_FIND_NEXT: usize = 2241;
const CMD_TRAY_SHOW: usize = 3001;
const CMD_HIDE_CATEGORIES: usize = 2214;
const CMD_SORT_BASE: usize = 2220;
const CMD_TOOLBAR: usize = 2230;
const CMD_COLUMNS: usize = 2231;
const CMD_LANGUAGE: usize = 2232;
const CMD_TUTORIALS: usize = 2233;
const CMD_TIP: usize = 2234;
const CMD_HOMEPAGE: usize = 2235;
const CMD_SUPPORT: usize = 2236;
const CMD_UPDATE: usize = 2237;
const CMD_TELL_FRIEND: usize = 2238;
const CMD_ORDER: usize = 2239;
const CMD_REGISTRATION: usize = 2240;

const COLUMNS: [(&str, i32); 8] = [
    ("Имя файла", 150),
    ("Очередь", 20),
    ("Размер", 75),
    ("Состояние", 65),
    ("Осталось времени", 75),
    ("Скорость передачи", 95),
    ("Дата последней попытки", 65),
    ("Описание", 178),
];

/// Категории IDM: (имя, корневая). Первые 5 — дети «Все загрузки».
const CATEGORIES: [(&str, bool); 10] = [
    ("Все загрузки", true),
    ("Сжатые", false),
    ("Документы", false),
    ("Музыка", false),
    ("Программы", false),
    ("Видео", false),
    ("Незавершённые", true),
    ("Завершённые", true),
    ("Проекты граббера", true),
    ("Очереди", true),
];

struct AppState {
    manager: DownloadManager,
    config: DownloadConfig,
    runtime: tokio::runtime::Runtime,
    hwnd: HWND,
    h_tree: HWND,
    h_list: HWND,
    h_status: HWND,
    h_toolbar: HWND,
    rows: Vec<JobInfo>,
    category: String,
    add_dialog: Option<HWND>,
    settings_dialog: Option<HWND>,
    last_clipboard: String,
    completed_seen: std::collections::HashSet<uuid::Uuid>,
    limit_start: Instant,
    limit_bytes: u64,
    limit_paused: bool,
    sched_active: Option<bool>,
    last_total: u64,
    sort_col: usize,
    sort_asc: bool,
    visible_ids: Vec<uuid::Uuid>,
    find_text: String,
}

unsafe impl Send for AppState {}

impl AppState {
    fn new() -> anyhow::Result<Self> {
        let config = DownloadConfig::load();
        let client = HttpClient::with_config(
            &config.user_agent,
            config.connect_timeout_secs,
            config.read_timeout_secs,
            config.proxy_url.as_deref(),
        );
        let client = match (config.proxy_user.as_deref(), config.proxy_pass.as_deref()) {
            (Some(user), Some(pass)) if !user.is_empty() => client.with_proxy_auth(user, pass),
            _ => client,
        };
        let manager = DownloadManager::new(config.clone(), client);
        let runtime = tokio::runtime::Runtime::new()?;
        Ok(Self {
            manager,
            config,
            runtime,
            hwnd: HWND::default(),
            h_tree: HWND::default(),
            h_list: HWND::default(),
            h_status: HWND::default(),
            h_toolbar: HWND::default(),
            rows: Vec::new(),
            category: "Все загрузки".to_string(),
            add_dialog: None,
            settings_dialog: None,
            last_clipboard: String::new(),
            completed_seen: HashSet::new(),
            limit_start: Instant::now(),
            limit_bytes: 0,
            limit_paused: false,
            sched_active: None,
            last_total: 0,
            sort_col: 6,
            sort_asc: false,
            visible_ids: Vec::new(),
            find_text: String::new(),
        })
    }

    fn refresh(&mut self) {
        self.rows = self.runtime.block_on(self.manager.list_jobs());
        unsafe { self.populate_list() };
    }

    /// Тик GUI: всё поведение из мануала IDM.
    fn on_tick(&mut self) {
        self.refresh();
        unsafe { self.handle_completions() };
        self.handle_clipboard();
        self.handle_scheduler();
        self.handle_limits();
    }

    /// Завершение: звук, антивирус, резервная копия, следующая в очереди.
    unsafe fn handle_completions(&mut self) {
        let completed: Vec<JobInfo> = self.rows.iter()
            .filter(|j| matches!(j.status, JobStatus::Completed) && !self.completed_seen.contains(&j.id))
            .cloned()
            .collect();
        for job in completed {
            self.completed_seen.insert(job.id);
            play_sound(&self.config.sound_complete);
            let name = PathBuf::from(&job.path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| job.url.clone());
            unsafe { tray_balloon(self.hwnd, "Загрузка завершена", &name) };
            if let Some(av) = self.config.antivirus_path.as_deref() {
                if !av.trim().is_empty() {
                    let mut cmd = std::process::Command::new(av);
                    if !self.config.antivirus_params.trim().is_empty() {
                        cmd.arg(self.config.antivirus_params.trim());
                    }
                    cmd.arg(&job.path);
                    let _ = cmd.spawn();
                }
            }
            if self.config.backup_enabled {
                if let Some(dir) = self.config.backup_dir.as_deref() {
                    let _ = std::fs::create_dir_all(dir);
                    if let Some(name) = PathBuf::from(&job.path).file_name() {
                        let _ = std::fs::copy(&job.path, dir.join(name));
                    }
                }
            }
        }
        if self.config.auto_start_next_in_queue {
            let next = self.rows.iter()
                .find(|j| matches!(j.status, JobStatus::Queued))
                .map(|j| j.id);
            if let Some(id) = next {
                let _ = self.runtime.block_on(self.manager.resume_job(id));
                self.refresh();
            }
        }
    }

    /// Монитор буфера обмена (вкладка «Общее»).
    fn handle_clipboard(&mut self) {
        if !self.config.watch_clipboard {
            return;
        }
        let text = arboard::Clipboard::new()
            .ok()
            .and_then(|mut c| c.get_text().ok())
            .unwrap_or_default();
        if text.is_empty() || text == self.last_clipboard {
            return;
        }
        self.last_clipboard = text.clone();
        // Alt + клик — запрет перехвата (мануал, Starting.htm)
        if self.config.capture_alt_key && (unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } as u16) & 0x8000 != 0 {
            return;
        }
        // Ctrl + клик — принудительный перехват (мануал, Starting.htm).
        let ctrl_force = self.config.capture_ctrl_key
            && (unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } as u16) & 0x8000 != 0;
        let added = self.add_urls(&text, ctrl_force);
        if added {
            self.refresh();
        }
    }

    /// Добавить все URL из текста: буфер обмена, пакетная загрузка, импорт.
    fn add_urls(&mut self, text: &str, ctrl_force: bool) -> bool {
        let mut added = false;
        for line in text.lines() {
            let url = line.trim();
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                continue;
            }
            if is_excluded(&self.config, url) && !ctrl_force {
                continue;
            }
            if !file_type_allowed(&self.config, url, ctrl_force) {
                continue;
            }
            let name = filename_from_url(url);
            let path = category_out_dir(&self.config, &PathBuf::from(&name)).join(&name);
            let login = site_login_for(&self.config, url);
            let job = DownloadJob {
                id: uuid::Uuid::new_v4(),
                url: url.to_string(),
                save_path: path,
                segment_count: self.config.default_segment_count.max(1),
                max_speed_bps: self.config.max_speed_bps,
                username: login.as_ref().map(|l| l.username.clone()).filter(|s| !s.is_empty()),
                password: login.as_ref().map(|l| l.password.clone()).filter(|s| !s.is_empty()),
                cookies: login.as_ref().map(|l| l.cookie.clone()).filter(|s| !s.is_empty()),
            };
            if self.runtime.block_on(self.manager.add_job(job)).is_ok() {
                added = true;
            }
        }
        added
    }

    /// Найти задачу по URL/пути (Ctrl+F / F3, как в IDM).
    unsafe fn find_job(&mut self, query: &str, next: bool) {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return;
        }
        let ids = self.visible_ids.clone();
        let current = unsafe { self.selected_job() }.map(|j| j.id);
        let offset = if next {
            ids.iter().position(|id| Some(*id) == current).map(|p| p + 1).unwrap_or(0)
        } else {
            0
        };
        let found = ids.iter().skip(offset)
            .find(|id| {
                self.rows.iter().find(|j| j.id == **id).is_some_and(|j| {
                    j.url.to_lowercase().contains(&q)
                        || j.path.to_string_lossy().to_lowercase().contains(&q)
                })
            })
            .copied()
            .or_else(|| {
                ids.iter().take(offset).find(|id| {
                    self.rows.iter().find(|j| j.id == **id).is_some_and(|j| {
                        j.url.to_lowercase().contains(&q)
                            || j.path.to_string_lossy().to_lowercase().contains(&q)
                    })
                }).copied()
            });
        if let Some(id) = found {
            if let Some(row) = ids.iter().position(|x| *x == id) {
                // LVITEMW.mask=LVIF_STATE, state=LVIS_SELECTED; LVM_SETITEMSTATE
                let mut item = LVITEMW {
                    mask: windows::Win32::UI::Controls::LVIF_STATE,
                    state: windows::Win32::UI::Controls::LVIS_SELECTED,
                    stateMask: windows::Win32::UI::Controls::LVIS_SELECTED,
                    ..Default::default()
                };
                SendMessageW(self.h_list, 0x102B, WPARAM(row), LPARAM(&mut item as *mut _ as isize));
            }
        }
    }

    /// Планировщик (scheduler.chm).
    fn handle_scheduler(&mut self) {
        use crate::download::state::parse_hms;
        use chrono::Timelike;
        let active = if self.config.sched_enabled {
            let now = chrono::Local::now();
            let cur = u64::from(now.hour() * 3600 + now.minute() * 60);
            let start = parse_hms(&self.config.sched_start).unwrap_or(0);
            let stop = parse_hms(&self.config.sched_stop).unwrap_or(86400);
            cur >= start && cur < stop
        } else {
            false
        };
        if self.sched_active != Some(active) {
            if active {
                self.runtime.block_on(self.manager.resume_all());
            } else {
                // Пауза, а не отмена: иначе resume_all() после окончания
                // периода не сможет возобновить задачи (мануал: расписание).
                self.runtime.block_on(self.manager.pause_all());
            }
            self.sched_active = Some(active);
            self.refresh();
        }
    }

    /// Ограничения по загрузке / квоты (вкладка «Соединение»).
    fn handle_limits(&mut self) {
        let total: u64 = self.rows.iter().map(|j| j.downloaded_bytes).sum();
        let delta = total.saturating_sub(self.last_total);
        self.last_total = total;
        if !self.config.download_limits_enabled || self.config.download_limit_mb == 0 {
            if self.limit_paused {
                self.runtime.block_on(self.manager.resume_all());
                self.limit_paused = false;
            }
            return;
        }
        let period_secs = self.config.download_limit_hours.max(1) * 3600;
        if self.limit_start.elapsed().as_secs() >= period_secs {
            self.limit_start = Instant::now();
            self.limit_bytes = 0;
            if self.limit_paused {
                self.runtime.block_on(self.manager.resume_all());
                self.limit_paused = false;
            }
        }
        self.limit_bytes += delta;
        let limit = self.config.download_limit_mb * 1_048_576;
        if self.limit_bytes >= limit && !self.limit_paused {
            self.runtime.block_on(self.manager.pause_all());
            self.limit_paused = true;
            message_box("Ограничения по загрузке", "Квота исчерпана. Загрузки приостановлены до конца периода.");
        }
    }

    unsafe fn populate_list(&mut self) {
        let h = self.h_list;
        SendMessageW(h, LVM_DELETEALLITEMS, WPARAM(0), LPARAM(0));

        let mut visible: Vec<(usize, &JobInfo)> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, j)| self.matches_category(j) && self.matches_status(j))
            .collect();

        let sort_col = self.sort_col;
        let sort_asc = self.sort_asc;
        visible.sort_by(|a, b| {
            let (ja, jb) = (a.1, b.1);
            let name_a = PathBuf::from(&ja.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let name_b = PathBuf::from(&jb.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let ord = match sort_col {
                0 => name_a.cmp(&name_b),
                2 => ja.total_bytes.cmp(&jb.total_bytes),
                3 => fmt_status(ja).cmp(&fmt_status(jb)),
                4 => fmt_left(ja).cmp(&fmt_left(jb)),
                5 => ja.speed_bps.cmp(&jb.speed_bps),
                6 => ja.path.cmp(&jb.path),
                7 => ja.url.cmp(&jb.url),
                _ => a.0.cmp(&b.0),
            };
            if sort_asc { ord } else { ord.reverse() }
        });

        // Как в IDM: завершённые сверху не выносятся — сохраняем порядок добавления.
        self.visible_ids = visible.iter().map(|(_, j)| j.id).collect();
        let mut row = 0i32;
        for (idx, job) in visible.drain(..) {
            let name = PathBuf::from(&job.path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| job.url.clone());

            let mut name_buf = wide(&name);
            let mut item = LVITEMW {
                mask: LVIF_TEXT | LVIF_PARAM,
                iItem: row,
                iSubItem: 0,
                pszText: PWSTR(name_buf.as_mut_ptr()),
                lParam: LPARAM(idx as isize),
                ..Default::default()
            };
            SendMessageW(
                h,
                LVM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&mut item as *mut _ as isize),
            );

            let values = [
                String::new(), // очередь
                fmt_size(job.total_bytes),
                fmt_status(job),
                fmt_left(job),
                fmt_speed(job.speed_bps),
                fmt_last_attempt(&job.path),
                host_of(&job.url),
            ];
            for (col, value) in values.iter().enumerate() {
                set_list_text(h, row, col as i32 + 1, value);
            }
            row += 1;
        }

        // Статус-бар: всего задач / скорость
        let speed = self.runtime.block_on(self.manager.total_speed());
        let text = format!(
            "Задач: {}    Скорость: {}",
            self.rows.len(),
            fmt_speed(speed)
        );
        set_status(self.h_status, &text);
    }

    fn matches_category(&self, job: &JobInfo) -> bool {
        match self.category.as_str() {
            "Все загрузки" => true,
            "Завершённые" => matches!(job.status, JobStatus::Completed),
            "Незавершённые" => matches!(
                job.status,
                JobStatus::Queued
                    | JobStatus::Downloading
                    | JobStatus::Paused
                    | JobStatus::Failed { .. }
                    | JobStatus::Cancelled
            ),
            "Очереди" => matches!(job.status, JobStatus::Queued),
            "Сжатые" => is_category(&job.path, &["zip", "rar", "7z", "gz", "tar", "iso"]),
            "Документы" => is_category(&job.path, &["doc", "docx", "pdf", "txt", "xls", "xlsx"]),
            "Музыка" => is_category(&job.path, &["mp3", "wav", "flac", "aac", "ogg", "m4a"]),
            "Программы" => is_category(&job.path, &["exe", "msi", "zip", "rar", "7z", "apk"]),
            "Видео" => is_category(&job.path, &["mp4", "mkv", "avi", "mov", "webm", "flv"]),
            _ => true,
        }
    }

    fn matches_status(&self, _job: &JobInfo) -> bool {
        true
    }

    unsafe fn selected_index(&self) -> Option<usize> {
        let count = SendMessageW(self.h_list, windows::Win32::UI::Controls::LVM_GETITEMCOUNT, WPARAM(0), LPARAM(0)).0;
        let mut item = LVITEMW::default();
        let sel = SendMessageW(
            self.h_list,
            windows::Win32::UI::Controls::LVM_GETNEXTITEM,
            WPARAM(usize::MAX),
            LPARAM(windows::Win32::UI::Controls::LVNI_SELECTED as isize),
        );
        if sel.0 < 0 || sel.0 >= count {
            return None;
        }
        item.mask = LVIF_PARAM;
        item.iItem = sel.0 as i32;
        item.pszText = PWSTR::null();
        let mut item = item;
        SendMessageW(
            self.h_list,
            windows::Win32::UI::Controls::LVM_GETITEMW,
            WPARAM(sel.0 as usize),
            LPARAM(&mut item as *mut _ as isize),
        );
        Some(item.lParam.0 as usize)
    }

    unsafe fn selected_job(&self) -> Option<JobInfo> {
        self.selected_index()
            .and_then(|i| self.rows.get(i).cloned())
    }

    fn handle_command(&mut self, id: usize) {
        let job = unsafe { self.selected_job() };
        match id {
            CMD_ADD => self.show_add_dialog(),
            CMD_START | CMD_RESUME => {
                if let Some(j) = job {
                    let _ = self.runtime.block_on(self.manager.resume_job(j.id));
                }
            }
            CMD_PAUSE | CMD_STOP => {
                if let Some(j) = job {
                    let _ = self.runtime.block_on(self.manager.pause_job(j.id));
                }
            }
            CMD_REMOVE => {
                if let Some(j) = job {
                    let _ = self.runtime.block_on(self.manager.remove_job(j.id));
                    self.refresh();
                }
            }
            CMD_START_ALL => {
                self.runtime.block_on(self.manager.resume_all());
                self.refresh();
            }
            CMD_STOP_ALL => {
                self.runtime.block_on(self.manager.stop_all());
                self.refresh();
            }
            CMD_CLEAR => {
                let ids: Vec<_> = self
                    .rows
                    .iter()
                    .filter(|j| matches!(j.status, JobStatus::Completed))
                    .map(|j| j.id)
                    .collect();
                for id in ids {
                    let _ = self.runtime.block_on(self.manager.remove_job(id));
                }
                self.refresh();
            }
            CMD_LAUNCH => {
                if let Some(j) = job {
                    unsafe { open_path(&j.path) };
                }
            }
            CMD_CTX_START | CMD_CTX_RESUME => {
                if let Some(j) = job {
                    let _ = self.runtime.block_on(self.manager.resume_job(j.id));
                    self.open_progress(j.id);
                    self.refresh();
                }
            }
            CMD_CTX_PAUSE => {
                if let Some(j) = job {
                    let _ = self.runtime.block_on(self.manager.pause_job(j.id));
                    self.refresh();
                }
            }
            CMD_CTX_CANCEL => {
                if let Some(j) = job {
                    let _ = self.runtime.block_on(self.manager.cancel_job(j.id));
                    self.refresh();
                }
            }
            CMD_CTX_REMOVE => {
                if let Some(j) = job {
                    let _ = self.runtime.block_on(self.manager.remove_job(j.id));
                    self.refresh();
                }
            }
            CMD_CTX_PROPERTIES => {
                if let Some(j) = job {
                    unsafe { show_properties(self.hwnd, &j) };
                }
            }
            CMD_CTX_OPEN_FOLDER => {
                if let Some(j) = job {
                    if let Some(parent) = PathBuf::from(&j.path).parent() {
                        unsafe { open_path(parent) };
                    }
                }
            }
            CMD_CTX_COPY_URL => {
                if let Some(j) = job {
                    let mut buf = wide(&j.url);
                    unsafe {
                        if arboard::Clipboard::new().is_ok_and(|mut c| c.set_text(j.url.clone()).is_ok()) {
                            // URL скопирован
                        }
                    }
                    let _ = buf;
                }
            }
            CMD_SETTINGS => self.show_settings_dialog(),
            CMD_SCHEDULE => {
                let initial = format!("{} {}", self.config.sched_start, self.config.sched_stop);
                if let Some(text) = unsafe { text_input_dialog(
                    self.hwnd, "Планировщик",
                    "Время начала и остановки (ЧЧ:ММ ЧЧ:ММ; пусто — выключить):",
                    &initial, false,
                ) } {
                    let parts: Vec<_> = text.split_whitespace().collect();
                    if parts.is_empty() {
                        self.config.sched_enabled = false;
                    } else if parts.len() >= 2 {
                        self.config.sched_start = parts[0].to_string();
                        self.config.sched_stop = parts[1].to_string();
                        self.config.sched_enabled = true;
                    }
                    self.sched_active = None;
                    self.config.save();
                }
            }
            CMD_ADD_BATCH => {
                if let Some(text) = unsafe { text_input_dialog(
                    self.hwnd, "Добавить групповую загрузку",
                    "Введите URL-адреса, по одному на строку:", "", true,
                ) } {
                    let added = self.add_urls(&text, true);
                    self.refresh();
                    message_box("Пакетная загрузка", &format!("Добавлено задач: {}", added));
                }
            }
            CMD_EXPORT => {
                let lines: Vec<String> = self.rows.iter()
                    .map(|j| format!("{}\t{}", j.url, j.path.display()))
                    .collect();
                if let Some(path) = unsafe { save_file_dialog(self.hwnd, "sda_downloads.txt") } {
                    match std::fs::write(&path, lines.join("\n")) {
                        Ok(()) => message_box("Экспорт", &format!("Экспортировано задач: {}", self.rows.len())),
                        Err(e) => message_box("Экспорт", &format!("Не удалось сохранить: {}", e)),
                    }
                }
            }
            CMD_IMPORT => {
                if let Some(path) = unsafe { open_file_dialog(self.hwnd) } {
                    match std::fs::read_to_string(&path) {
                        Ok(text) => {
                            let added = self.add_urls(&text, true);
                            self.refresh();
                            message_box("Импорт", &format!("Импортировано URL: {}", added));
                        }
                        Err(e) => message_box("Импорт", &format!("Не удалось прочитать: {}", e)),
                    }
                }
            }
            CMD_FIND => {
                if let Some(query) = unsafe { text_input_dialog(
                    self.hwnd, "Найти", "Имя файла, адрес или путь:", &self.find_text, false,
                ) } {
                    self.find_text = query.clone();
                    unsafe { self.find_job(&query, false) };
                }
            }
            CMD_FIND_NEXT => {
                let query = self.find_text.clone();
                unsafe { self.find_job(&query, true) };
            }
            CMD_GRABBER => {
                message_box("Граббер", "Перехват ссылок из браузера через расширение SDA");
            }
            CMD_ABOUT => {
                message_box("О программе", "SDA — Super Download Agent\nКлон IDM на Rust (Win32 API)");
            }
            CMD_EXIT => unsafe { PostQuitMessage(0) },
            CMD_TRAY_SHOW => unsafe { ShowWindow(self.hwnd, SW_SHOW); },
            CMD_ADD_BATCH_CLIP => {
                self.last_clipboard.clear();
                self.handle_clipboard();
            }
            CMD_GRABBER_SITE | CMD_DROP_TARGET | CMD_REDOWNLOAD | CMD_MOVE_RENAME
            | CMD_SPEED_LIMIT | CMD_TOOLBAR | CMD_COLUMNS | CMD_LANGUAGE | CMD_TUTORIALS | CMD_TIP
            | CMD_HOMEPAGE | CMD_SUPPORT | CMD_UPDATE | CMD_TELL_FRIEND
            | CMD_ORDER | CMD_REGISTRATION => {
                message_box("SDA", "Функция доступна в полной версии IDM");
            }
            CMD_PAUSE_ALL => {
                self.runtime.block_on(self.manager.pause_all());
                self.refresh();
            }
            CMD_DELETE_COMPLETED => {
                let ids: Vec<_> = self.rows.iter().filter(|j| matches!(j.status, JobStatus::Completed)).map(|j| j.id).collect();
                for id in ids { let _ = self.runtime.block_on(self.manager.remove_job(id)); }
                self.refresh();
            }
            CMD_START_QUEUE | CMD_RESUME => {
                self.runtime.block_on(self.manager.resume_all());
                self.refresh();
            }
            CMD_STOP_QUEUE => {
                self.runtime.block_on(self.manager.stop_all());
                self.refresh();
            }
            CMD_HIDE_CATEGORIES => {
                unsafe {
                    let show = windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(self.h_tree);
                    ShowWindow(self.h_tree, if show.0 != 0 { windows::Win32::UI::WindowsAndMessaging::SW_HIDE } else { SW_SHOW });
                }
            }
            id if id >= CMD_SORT_BASE && id < CMD_SORT_BASE + 10 => {
                self.sort_col = id - CMD_SORT_BASE;
                self.sort_asc = true;
                self.refresh();
            }
            CMD_CAT_ALL | CMD_CAT_COMPLETED | CMD_CAT_UNFINISHED | CMD_CAT_QUEUE => {
                self.category = match id {
                    CMD_CAT_COMPLETED => "Завершённые".to_string(),
                    CMD_CAT_UNFINISHED => "Незавершённые".to_string(),
                    CMD_CAT_QUEUE => "Очереди".to_string(),
                    _ => "Все загрузки".to_string(),
                };
                self.refresh();
            }
            _ => {}
        }
    }

    unsafe fn on_notify(&mut self, lparam: LPARAM) {
        let nm = &*(lparam.0 as *const NMHDR);
        if nm.code == HDN_ITEMCLICKW {
            let header = &*(lparam.0 as *const NMHEADERW);
            let col = header.iItem.max(0) as usize;
            if col == self.sort_col {
                self.sort_asc = !self.sort_asc;
            } else {
                self.sort_col = col;
                self.sort_asc = true;
            }
            self.refresh();
            return;
        }
        if nm.code == NM_DBLCLK && nm.idFrom == ID_LIST as usize {
            if let Some(job) = self.selected_job() {
                open_path(&job.path);
            }
        }
        if nm.idFrom == ID_TREE as usize {
            // TVN_SELCHANGED (все variant-коды) — просто перечитать выбранную категорию.
            let hitem = SendMessageW(
                self.h_tree,
                windows::Win32::UI::Controls::TVM_GETNEXTITEM,
                WPARAM(windows::Win32::UI::Controls::TVGN_CARET as usize),
                LPARAM(0),
            );
            if hitem.0 != 0 {
                let mut buf = [0u16; 128];
                let mut tvi = TVITEMW {
                    mask: TVIF_TEXT,
                    hItem: windows::Win32::UI::Controls::HTREEITEM(hitem.0),
                    pszText: PWSTR(buf.as_mut_ptr()),
                    cchTextMax: buf.len() as i32,
                    ..Default::default()
                };
                SendMessageW(
                    self.h_tree,
                    TVM_GETITEMW,
                    WPARAM(0),
                    LPARAM(&mut tvi as *mut _ as isize),
                );
                let text = String::from_utf16_lossy(&buf);
                let text = text.trim_end_matches('\0');
                if CATEGORIES.iter().any(|(c, _)| *c == text) {
                    self.category = text.to_string();
                    self.refresh();
                }
            }
        }
    }

    fn show_add_dialog(&mut self) {
        if self.add_dialog.map(|h| h.0 != 0).unwrap_or(false) {
            return;
        }
        if let Some(result) = show_add_dialog(self.hwnd) {
            if is_excluded(&self.config, &result.url) {
                message_box("Загрузка", "Сайт или URL исключён в настройках «Типы Файлов».");
                return;
            }
            let save_path = if result.save_path.trim().is_empty() {
                let name = filename_from_url(&result.url);
                category_out_dir(&self.config, &PathBuf::from(&name)).join(&name)
            } else {
                PathBuf::from(result.save_path.trim())
            };
            // «Запомнить последний путь категории» (вкладка «Сохранение»).
            if self.config.remember_last_path && !result.save_path.trim().is_empty() {
                let category = crate::download::category_for_path(&save_path).to_string();
                if let Some(folder) = self.config.category_folders.iter_mut().find(|f| f.category.eq_ignore_ascii_case(&category)) {
                    folder.path = save_path.clone();
                } else {
                    self.config.category_folders.push(crate::download::state::CategoryFolder {
                        category,
                        path: save_path.clone(),
                    });
                }
                self.config.save();
            }
            let login = site_login_for(&self.config, &result.url);
            let job = DownloadJob {
                id: uuid::Uuid::new_v4(),
                url: result.url,
                save_path,
                segment_count: self.config.default_segment_count.max(1),
                max_speed_bps: self.config.max_speed_bps,
                username: login.as_ref().map(|l| l.username.clone()).filter(|s| !s.is_empty()),
                password: login.as_ref().map(|l| l.password.clone()).filter(|s| !s.is_empty()),
                cookies: login.as_ref().map(|l| l.cookie.clone()).filter(|s| !s.is_empty()),
            };
            match self.runtime.block_on(self.manager.add_job(job)) {
                Ok(id) => {
                    if result.action == AddAction::Later {
                        let _ = self.runtime.block_on(self.manager.pause_job(id));
                    } else {
                        self.open_progress(id);
                    }
                    unsafe { play_sound(&self.config.sound_start) };
                    self.refresh();
                }
                Err(e) => {
                    unsafe { play_sound(&self.config.sound_error) };
                    message_box("Ошибка", &e.to_string())
                }
            }
        }
    }

    fn open_progress(&mut self, id: uuid::Uuid) {
        if self.config.progress_dialog == "show" {
            unsafe { open_progress_dialog(self.hwnd, self as *mut _, id) };
        }
    }

    fn show_settings_dialog(&mut self) {
        if self.settings_dialog.map(|h| h.0 != 0).unwrap_or(false) {
            return;
        }
        if let Some(config) = show_settings_dialog(self.hwnd, &self.config) {
            unsafe { let _ = set_start_with_windows(config.start_with_windows); };
            self.runtime.block_on(self.manager.update_config(config.clone()));
            self.config = config;
            self.config.save();
            let client = HttpClient::with_config(
                &self.config.user_agent,
                self.config.connect_timeout_secs,
                self.config.read_timeout_secs,
                self.config.proxy_url.as_deref(),
            );
            let client = match (self.config.proxy_user.as_deref(), self.config.proxy_pass.as_deref()) {
                (Some(user), Some(pass)) if !user.is_empty() => client.with_proxy_auth(user, pass),
                _ => client,
            };
            self.manager.set_client(client);
            self.refresh();
        }
    }
}

/// Иконка в системном лотке (аналог IDM tray).
unsafe fn tray_add(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = std::mem::zeroed();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = WM_APP + 1;
    if let Ok(exe) = std::env::current_exe() {
        let path = exe.parent().unwrap_or(std::path::Path::new("")).join("app.ico");
        if let Ok(handle) = LoadImageW(None, windows::core::PCWSTR(wide(&path.to_string_lossy()).as_ptr()), IMAGE_ICON, 16, 16, LR_LOADFROMFILE) {
            data.hIcon = HICON(handle.0);
        }
    }
    let tip = wide("SDA — Super Download Agent");
    for (i, c) in tip.iter().enumerate().take(data.szTip.len() - 1) {
        data.szTip[i] = *c;
    }
    Shell_NotifyIconW(NIM_ADD, &data);
}

unsafe fn tray_balloon(hwnd: HWND, title: &str, text: &str) {
    let mut data: NOTIFYICONDATAW = std::mem::zeroed();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_INFO;
    data.dwInfoFlags = NIIF_INFO;
    for (i, c) in wide(title).iter().enumerate().take(data.szInfoTitle.len() - 1) {
        data.szInfoTitle[i] = *c;
    }
    for (i, c) in wide(text).iter().enumerate().take(data.szInfo.len() - 1) {
        data.szInfo[i] = *c;
    }
    Shell_NotifyIconW(NIM_MODIFY, &data);
}

unsafe fn tray_remove(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = std::mem::zeroed();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &data);
}

/// Запуск SDA при входе в Windows (вкладка «Общее»).
unsafe fn set_start_with_windows(enabled: bool) -> anyhow::Result<()> {
    let mut key = windows::Win32::System::Registry::HKEY::default();
    RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
        0,
        KEY_WRITE,
        &mut key,
    )?;
    if enabled {
        let exe = std::env::current_exe()?;
        let mut value = wide(&format!("\"{}\"", exe.to_string_lossy()));
        let bytes: &[u8] = std::slice::from_raw_parts(value.as_ptr() as *const u8, (value.len() - 1) * 2);
        RegSetValueExW(key, w!("SDA"), 0, REG_SZ, Some(bytes))?;
    } else {
        let _ = RegDeleteValueW(key, w!("SDA"));
    }
    RegCloseKey(key)?;
    Ok(())
}

unsafe fn play_sound(path: &Option<String>) {
    if let Some(path) = path {
        if !path.is_empty() {
            let mut buf = wide(path);
            PlaySoundW(windows::core::PCWSTR(buf.as_mut_ptr()), None, SND_FILENAME | SND_ASYNC);
        }
    }
}

/// Авторизация сайта по мануалу (вкладка «Пароли сайтов»).
fn site_login_for(config: &DownloadConfig, url: &str) -> Option<crate::download::state::SiteLogin> {
    config.site_logins.iter().find(|l| {
        let prefix = l.url.trim_end_matches('/');
        !prefix.is_empty() && url.starts_with(prefix)
    }).cloned()
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1)
        .unwrap_or(url)
        .split('/').next()
        .unwrap_or("")
        .to_string()
}

fn is_excluded(config: &DownloadConfig, url: &str) -> bool {
    let host = host_of(url);
    config.excluded_sites.iter().any(|s| {
        let pat = s.trim().trim_start_matches("*");
        !pat.is_empty() && host.ends_with(pat)
    }) || config.excluded_urls.iter().any(|u| url.starts_with(u.trim()))
}

/// Типы файлов для автоперехвата (вкладка «Типы Файлов»).
fn file_type_allowed(config: &DownloadConfig, url: &str, force: bool) -> bool {
    if force {
        return true;
    }
    let name = filename_from_url(url);
    let ext = PathBuf::from(&name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .trim_start_matches('.')
        .to_lowercase();
    ext.is_empty() || config.file_types.iter().any(|e| {
        e.trim().trim_start_matches('.').eq_ignore_ascii_case(&ext)
    })
}

fn category_out_dir(config: &DownloadConfig, path: &std::path::Path) -> PathBuf {
    let category = crate::download::category_for_path(path);
    config.category_folders.iter()
        .find(|f| f.category.eq_ignore_ascii_case(category))
        .map(|f| f.path.clone())
        .unwrap_or_else(|| config.output_dir.clone())
}

fn nonempty(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn set_list_text(h: HWND, row: i32, col: i32, text: &str) {
    let mut buf = wide(text);
    let mut item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: row,
        iSubItem: col,
        pszText: PWSTR(buf.as_mut_ptr()),
        ..Default::default()
    };
    SendMessageW(h, LVM_SETITEMW, WPARAM(0), LPARAM(&mut item as *mut _ as isize));
}

unsafe fn set_status(h: HWND, text: &str) {
    let mut buf = wide(text);
    SendMessageW(h, SB_SETTEXTW, WPARAM(0), LPARAM(buf.as_mut_ptr() as isize));
}

fn fmt_size(n: u64) -> String {
    if n == 0 {
        return String::new();
    }
    const K: f64 = 1024.0;
    if n < K as u64 {
        format!("{} Б", n)
    } else if n < (K * K) as u64 {
        format!("{:.2} КБ", n as f64 / K)
    } else if n < (K * K * K) as u64 {
        format!("{:.2} МБ", n as f64 / (K * K))
    } else {
        format!("{:.2} ГБ", n as f64 / (K * K * K))
    }
}

fn fmt_speed(bps: u64) -> String {
    if bps == 0 {
        return String::new();
    }
    format!("{}/с", fmt_size(bps))
}

fn fmt_status(j: &JobInfo) -> String {
    match &j.status {
        JobStatus::Queued => "Очередь".to_string(),
        JobStatus::Downloading => {
            if j.total_bytes > 0 {
                format!(
                    "{:.2}%",
                    j.downloaded_bytes as f64 / j.total_bytes as f64 * 100.0
                )
            } else {
                "Загрузка".to_string()
            }
        }
        JobStatus::Paused => "Остановлено".to_string(),
        JobStatus::Completed => "Готово".to_string(),
        JobStatus::Failed { .. } => "Ошибка".to_string(),
        JobStatus::Cancelled => "Отменено".to_string(),
    }
}

fn fmt_left(j: &JobInfo) -> String {
    if j.speed_bps == 0 || j.total_bytes <= j.downloaded_bytes {
        return String::new();
    }
    let secs = (j.total_bytes - j.downloaded_bytes) / j.speed_bps;
    if secs >= 3600 {
        format!("{} ч {} мин", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{} мин {} с", secs / 60, secs % 60)
    } else {
        format!("{} с", secs)
    }
}

fn fmt_date() -> String {
    use chrono::Local;
    Local::now().format("%d.%m.%Y %H:%M").to_string()
}

/// «Дата последней попытки» — время последнего изменения файла.
fn fmt_last_attempt(path: &std::path::Path) -> String {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y %H:%M").to_string())
        .unwrap_or_default()
}

fn is_category(path: &std::path::Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn message_box(title: &str, text: &str) {
    unsafe {
        let mut t = wide(text);
        let mut c = wide(title);
        MessageBoxW(
            HWND::default(),
            PCWSTR(t.as_mut_ptr()),
            PCWSTR(c.as_mut_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MB_OK,
        );
    }
}

/// Перевод фокуса по WS_TABSTOP-контролам: наши окна создаются не через
/// менеджер диалогов, поэтому DefWindowProc не обрабатывает WM_NEXTDLGCTL.
unsafe fn next_tab_stop(parent: HWND, forward: bool) {
    let mut stops: Vec<HWND> = Vec::new();
    let mut child = GetWindow(parent, GW_CHILD);
    while child.0 != 0 {
        if (GetWindowLongPtrW(child, GWL_STYLE) & WS_TABSTOP.0 as isize) != 0 {
            stops.push(child);
        }
        child = GetWindow(child, GW_HWNDNEXT);
    }
    if stops.is_empty() {
        return;
    }
    let current = GetFocus();
    let idx = stops.iter().position(|h| *h == current);
    let next = match idx {
        Some(i) if forward => (i + 1) % stops.len(),
        Some(i) => (i + stops.len() - 1) % stops.len(),
        None => 0,
    };
    SetFocus(stops[next]);
}

unsafe fn handle_next_dlgctl(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if lparam.0 != 0 {
        SetFocus(HWND(wparam.0 as isize));
    } else {
        next_tab_stop(hwnd, wparam.0 == 0);
    }
    LRESULT(0)
}

/// Собственная обработка клавиш модального окна. IsDialogMessageW на обычных
/// (не dialog-manager) окнах поглощает WM_CHAR и ввод в Edit не работает.
unsafe fn modal_key(
    hwnd: HWND,
    msg: &windows::Win32::UI::WindowsAndMessaging::MSG,
    multiline: bool,
) -> bool {
    if msg.message != windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN {
        return false;
    }
    match msg.wParam.0 as i32 {
        9 => {
            // Tab / Shift+Tab — переход по WS_TABSTOP-контролам.
            let shift = (GetAsyncKeyState(0x10) as u16) & 0x8000 != 0;
            SendMessageW(hwnd, 0x0028, WPARAM(if shift { 1 } else { 0 }), LPARAM(0));
            true
        }
        13 if !multiline => {
            // Enter — нажатие default-кнопки (HIWORD=DC_HASDEFID, LOWORD=ид).
            let r = SendMessageW(hwnd, 0x0400, WPARAM(0), LPARAM(0)).0 as isize;
            if (r >> 16) & 0xffff == DC_HASDEFID as isize {
                let id = r & 0xffff;
                SendMessageW(hwnd, 0x0111, WPARAM(id as usize), LPARAM(0));
                true
            } else {
                false
            }
        }
        0x56 if (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0 => {
            // Ctrl+V — вставляем текст буфера обмена в фокусированный Edit.
            if let Ok(mut clip) = arboard::Clipboard::new() {
                if let Ok(text) = clip.get_text() {
                    let mut b = wide(&text);
                    SendMessageW(GetFocus(), 0x000C, WPARAM(0), LPARAM(b.as_mut_ptr() as isize));
                    return true;
                }
            }
            false
        }
        27 => {
            // Escape — закрытие диалога через WM_CLOSE.
            SendMessageW(hwnd, 0x0010, WPARAM(0), LPARAM(0));
            true
        }
        _ => false,
    }
}

/// Выбор папки через SHBrowseForFolderW (кнопки «Обзор...»).
unsafe fn browse_folder(owner: HWND) -> Option<String> {
    let title = wide("Выберите папку");
    let mut bi: BROWSEINFOW = std::mem::zeroed();
    bi.hwndOwner = owner;
    bi.lpszTitle = PCWSTR(title.as_ptr());
    bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
    let pidl = SHBrowseForFolderW(&bi);
    if pidl.is_null() {
        return None;
    }
    let mut path = [0u16; 260];
    let ok = SHGetPathFromIDListW(pidl as *const _, &mut path);
    CoTaskMemFree(Some(pidl as *const c_void));
    if ok.0 == 0 {
        return None;
    }
    let s = String::from_utf16_lossy(&path).trim_end_matches('\0').to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Стандартный Save As (GetSaveFileNameW) с предупреждением о существовании
/// файла (кнопки «Сохранить как» в диалоге 130).
unsafe fn save_file_dialog(owner: HWND, initial: &str) -> Option<String> {
    let mut buf = wide(initial);
    // GetSaveFileNameW записывает результат в lpstrFile — буфер должен быть
    // не меньше MAX_PATH даже если исходное поле было пустым.
    buf.resize(buf.len().max(260), 0);
    let mut filter = wide("Все файлы\0*.*\0"); // wide() добавит второй \0
    let initial_dir = std::path::Path::new(initial)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| wide(&p.to_string_lossy()));
    let mut ofn: OPENFILENAMEW = std::mem::zeroed();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = PCWSTR(filter.as_ptr());
    ofn.lpstrFile = PWSTR(buf.as_mut_ptr());
    ofn.nMaxFile = buf.len() as u32;
    if let Some(dir) = &initial_dir {
        ofn.lpstrInitialDir = PCWSTR(dir.as_ptr());
    }
    ofn.nFilterIndex = 1;
    ofn.Flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
    if GetSaveFileNameW(&mut ofn).0 == 0 {
        return None;
    }
    let s = String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Открытие текстового файла (импорт списка загрузок, мануал Import_Downloads.htm).
unsafe fn open_file_dialog(owner: HWND) -> Option<String> {
    let mut buf = vec![0u16; 32768];
    let mut filter = wide("Текстовые файлы\0*.txt\0");
    let mut ofn: OPENFILENAMEW = std::mem::zeroed();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = PCWSTR(filter.as_ptr());
    ofn.lpstrFile = PWSTR(buf.as_mut_ptr());
    ofn.nMaxFile = buf.len() as u32;
    ofn.Flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR;
    if GetOpenFileNameW(&mut ofn).0 == 0 {
        return None;
    }
    let s = String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string();
    if s.is_empty() { None } else { Some(s) }
}

struct InputDlgState {
    done: bool,
    result: Option<String>,
    h_edit: HWND,
    multiline: bool,
}

unsafe extern "system" fn input_dlg_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut InputDlgState;
    if msg == 0x0028 {
        return handle_next_dlgctl(hwnd, wparam, lparam);
    }
    if msg == 0x0006 && (wparam.0 & 0xffff) != 0 {
        if let Some(s) = state.as_mut() { SetFocus(s.h_edit); }
    }
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
            if let Some(s) = state.as_mut() {
                match wparam.0 as i32 & 0xffff {
                    1 => {
                        s.result = Some(get_text(s.h_edit));
                        s.done = true;
                        DestroyWindow(hwnd);
                    }
                    2 => {
                        s.done = true;
                        DestroyWindow(hwnd);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        0x0400 => {
            // DM_GETDEFID: в многострочном поле Enter — новая строка, не ОК.
            if state.as_ref().map(|s| s.multiline).unwrap_or(false) {
                return LRESULT(0);
            }
            return LRESULT(((DC_HASDEFID as isize) << 16) | 1);
        }
        windows::Win32::UI::WindowsAndMessaging::WM_CLOSE => {
            if let Some(s) = state.as_mut() { s.done = true; }
            DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Универсальный модальный диалог ввода (пакетная загрузка, поиск, расписание).
unsafe fn text_input_dialog(
    parent: HWND,
    title: &str,
    label: &str,
    initial: &str,
    multiline: bool,
) -> Option<String> {
    static mut REGISTERED: bool = false;
    if !REGISTERED {
        let hinstance = GetModuleHandleW(None).ok()?;
        let mut class: WNDCLASSEXW = std::mem::zeroed();
        class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        class.lpfnWndProc = Some(input_dlg_proc);
        class.hInstance = HINSTANCE(hinstance.0);
        class.hCursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        class.hbrBackground = HBRUSH((windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as isize);
        class.lpszClassName = w!("SDAInputDlg");
        RegisterClassExW(&class);
        REGISTERED = true;
    }
    let mut t = wide(title);
    let h = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0x2), // WS_EX_DLGMODALFRAME
        w!("SDAInputDlg"), PCWSTR(t.as_ptr()),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !(0x20000 | 0x40000)),
        200, 160, if multiline { 580 } else { 480 }, if multiline { 310 } else { 160 },
        parent, windows::Win32::UI::WindowsAndMessaging::HMENU(0),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0), None,
    );
    if h.0 == 0 {
        return None;
    }
    create_label(h, label, 15, 15, 540, 20);
    let style = if multiline {
        WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | 0x4 | 0x80 | 0x1000
    } else {
        WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32
    };
    let edit = CreateWindowExW(
        WS_EX_CLIENTEDGE, w!("Edit"), w!(""),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(style),
        15, 45, if multiline { 550 } else { 450 }, if multiline { 210 } else { 22 },
        h, windows::Win32::UI::WindowsAndMessaging::HMENU(1),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0), None,
    );
    let mut b = wide(initial);
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(edit, PCWSTR(b.as_mut_ptr()));
    let base_y = if multiline { 270 } else { 120 };
    create_button(h, "OK", 1, if multiline { 410 } else { 310 }, base_y, 80, 26);
    create_button(h, "Отмена", 2, if multiline { 495 } else { 395 }, base_y, 80, 26);

    let mut state = InputDlgState { done: false, result: None, h_edit: edit, multiline };
    SetWindowLongPtrW(h, GWLP_USERDATA, &mut state as *mut _ as isize);
    ShowWindow(h, SW_SHOW);
    UpdateWindow(h);
    EnableWindow(parent, FALSE);
    SetForegroundWindow(h);
    SetFocus(edit);
    let multiline = state.multiline;
    let mut msg = std::mem::zeroed();
    while !state.done && GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
        if !modal_key(h, &msg, multiline) {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    EnableWindow(parent, TRUE);
    SetForegroundWindow(parent);
    state.result
}

unsafe fn open_path(path: &std::path::Path) {
    let p = wide(&path.to_string_lossy());
    ShellExecuteW(HWND::default(), w!("open"), PCWSTR(p.as_ptr()), None, None, windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL);
}

// ---------------------------------------------------------------------------
// Окно добавления задачи
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum AddAction {
    Start,
    Later,
}

/// Диалог 130 «Информация о загружаемом файле» (Download File Info).
struct AddResult {
    url: String,
    save_path: String,
    description: String,
    action: AddAction,
}

struct AddDialogState {
    done: bool,
    result: Option<AddResult>,
    h_url: HWND,
    h_cat: HWND,
    h_save: HWND,
    h_remember: HWND,
    h_path: HWND,
    h_desc: HWND,
}

unsafe extern "system" fn add_dialog_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AddDialogState;
    if msg == 0x0028 {
        return handle_next_dlgctl(hwnd, wparam, lparam);
    }
    if msg == 0x0400 {
        return LRESULT(((DC_HASDEFID as isize) << 16) | 1006); // DM_GETDEFID
    }
    if msg == 0x0006 && (wparam.0 & 0xffff) != 0 {
        if let Some(s) = state.as_mut() { SetFocus(s.h_url); }
    }
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
            if let Some(s) = state.as_mut() {
                match wparam.0 as i32 & 0xffff {
                    1006 | 1004 => {
                        s.result = Some(AddResult {
                            url: get_text(s.h_url),
                            save_path: get_text(s.h_save),
                            description: get_text(s.h_desc),
                            action: if (wparam.0 as i32 & 0xffff) == 1006 { AddAction::Start } else { AddAction::Later },
                        });
                        s.done = true;
                        DestroyWindow(hwnd);
                    }
                    2 => {
                        s.done = true;
                        DestroyWindow(hwnd);
                    }
                    1697 => message_box("Категория", "Новая категория добавляется в дереве категорий."),
                    1608 => {
                        if let Some(s) = state.as_mut() {
                            let cur = get_text(s.h_save);
                            if let Some(p) = save_file_dialog(hwnd, &cur) {
                                let mut b = wide(&p);
                                let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(s.h_save, PCWSTR(b.as_mut_ptr()));
                            }
                        }
                    }
                    1173 => message_box("Предосмотр", "Предосмотр доступен для архивов."),
                    _ => {}
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_CLOSE => {
            if let Some(s) = state.as_mut() {
                s.done = true;
            }
            DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn get_text(h: HWND) -> String {
    let len = windows::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(h) as usize;
    let mut buf = vec![0u16; len + 2];
    windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(h, &mut buf);
    String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string()
}

unsafe fn create_edit(parent: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("Edit"),
        w!(""),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
            WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32,
        ),
        x, y, w, h, parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    )
}

unsafe fn create_label(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
    let mut buf = wide(text);
    CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        w!("Static"),
        PCWSTR(buf.as_mut_ptr()),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
        x, y, w, h, parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(0),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    )
}

unsafe fn create_button(parent: HWND, text: &str, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    let mut buf = wide(text);
    CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        w!("Button"),
        PCWSTR(buf.as_mut_ptr()),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0),
        x, y, w, h, parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    )
}

unsafe fn create_combo(parent: HWND, id: i32, x: i32, y: i32, w: i32, h: i32, editable: bool) -> HWND {
    let style = if editable { 0x2 | 0x40 } else { 0x1 }; // CBS_DROPDOWN | CBS_AUTOHSCROLL
    CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("ComboBox"),
        w!(""),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
        x, y, w, h, parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    )
}

fn show_add_dialog(parent: HWND) -> Option<AddResult> {
    unsafe {
        let hinstance = GetModuleHandleW(None).ok()?;
        let mut class: WNDCLASSEXW = std::mem::zeroed();
        class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        class.lpfnWndProc = Some(add_dialog_proc);
        class.hInstance = hinstance.into();
        class.hCursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        class.hbrBackground = HBRUSH((windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as isize);
        class.lpszClassName = w!("SDAAddDialog");
        RegisterClassExW(&class);

        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0x2), // WS_EX_DLGMODALFRAME
            w!("SDAAddDialog"),
            w!("Информация о загружаемом файле"),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !(0x20000 | 0x40000)),
            667, 396, 586, 247,
            parent,
            windows::Win32::UI::WindowsAndMessaging::HMENU(0),
            HINSTANCE(hinstance.0),
            None,
        );
        if hwnd.0 == 0 {
            return None;
        }

        let mut state = AddDialogState {
            done: false,
            result: None,
            h_url: create_edit(hwnd, 1007, 104, 36, 366, 23),
            h_cat: create_combo(hwnd, 1209, 104, 67, 174, 21, false),
            h_save: create_combo(hwnd, 1008, 104, 94, 335, 21, true),
            h_remember: create_button(hwnd, "Запомнить этот путь для категории «Общие»", 1609, 112, 120, 230, 16),
            h_path: create_edit(hwnd, 1815, 112, 143, 348, 23),
            h_desc: create_edit(hwnd, 1009, 104, 179, 366, 23),
        };
        create_label(hwnd, "URL", 16, 39, 81, 16);
        create_label(hwnd, "Категория", 16, 67, 81, 16);
        create_label(hwnd, "Сохранить как", 16, 98, 81, 16);
        create_label(hwnd, "Описание", 16, 182, 81, 16);
        create_button(hwnd, "+", 1697, 283, 65, 24, 23);
        create_button(hwnd, "...", 1608, 446, 94, 24, 23);
        create_button(hwnd, "Предосмотр", 1173, 487, 158, 75, 23);
        create_button(hwnd, "Скачать позже", 1004, 104, 211, 111, 23);
        create_button(hwnd, "Начать качать", 1006, 232, 211, 111, 23);
        create_button(hwnd, "Отмена", 2, 359, 210, 111, 23);
        for cat in ["Общие", "Сжатые", "Документы", "Музыка", "Программы", "Видео"] {
            let mut buf = wide(cat);
            SendMessageW(state.h_cat, 0x0143, WPARAM(0), LPARAM(buf.as_mut_ptr() as isize));
        }
        SendMessageW(state.h_cat, 0x014E, WPARAM(0), LPARAM(0)); // CB_SETCURSEL 0

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut state as *mut _ as isize);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        EnableWindow(parent, FALSE);
        SetForegroundWindow(hwnd);
        SetFocus(state.h_url);
        let mut msg = std::mem::zeroed();
        while !state.done && GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
            if !modal_key(hwnd, &msg, false) {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        EnableWindow(parent, TRUE);
        SetForegroundWindow(parent);
        state.result
    }
}

// ---------------------------------------------------------------------------
// Окно настроек (с вкладками, как в IDM)
// ---------------------------------------------------------------------------

struct SettingsState {
    done: bool,
    result: Option<DownloadConfig>,
    config: DownloadConfig,
    h_tab: HWND,
    pages: Vec<HWND>,
    extras: Vec<ExtraControl>,
    h_out: HWND,
    h_out_browse: HWND,
    h_seg: HWND,
    h_conc: HWND,
    h_watch: HWND,
    h_speed: HWND,
    h_retry: HWND,
    h_conn: HWND,
    h_read: HWND,
    h_proxy: HWND,
    h_sched: HWND,
    h_start: HWND,
    h_stop: HWND,
}

#[derive(Clone, Copy, PartialEq)]
enum ExtraKind {
    Text,
    Number,
    Bool,
}

#[derive(Clone, Copy)]
enum ExtraKey {
    StartWin, BrowserIntegration, CtrlKey, AltKey, DetectApps,
    FileTypes, ExcludedSites, ExcludedUrls,
    ConnType, ConnExceptions, LimitsEnabled, LimitMb, LimitHours,
    CategoryFolders, RememberPath,
    ProgressDialog, ShowStart, ShowComplete, Silent, StartNow, IgnoreDate, AvPath, AvParams,
    ProxyUser, ProxyPass, FtpPasv, UseIEProxy, ProxyExclusions,
    SiteLogins, DialupConn, DialupUser, DialupPass, DialupSave, DialupAttempts, DialupInterval,
    SoundComplete, SoundError, SoundStart,
    AutoNextQueue, ToolbarStyle, VisibleColumns,
    BackupEnabled, BackupDir, ScanDownloads,
}

struct ExtraControl {
    id: i32,
    hwnd: HWND,
    key: ExtraKey,
    kind: ExtraKind,
}

const SIC_OUT: i32 = 3101;
const SIC_OUT_BROWSE: i32 = 3195;
const SIC_PROXY_IE: i32 = 3198;
const SIC_SEG: i32 = 3102;
const SIC_CONC: i32 = 3103;
const SIC_WATCH: i32 = 3104;
const SIC_SPEED: i32 = 3105;
const SIC_RETRY: i32 = 3106;
const SIC_CONN: i32 = 3107;
const SIC_READ: i32 = 3108;
const SIC_PROXY: i32 = 3109;
const SIC_SCHED: i32 = 3110;
const SIC_START: i32 = 3111;
const SIC_STOP: i32 = 3112;
const SIC_OK: i32 = 3113;
const SIC_CANCEL: i32 = 3114;
const SIC_TEST_SOUND: i32 = 3197;

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsState;
    if msg == 0x0028 {
        return handle_next_dlgctl(hwnd, wparam, lparam);
    }
    if msg == 0x0400 {
        return LRESULT(((DC_HASDEFID as isize) << 16) | SIC_OK as isize);
    }
    if msg == 0x0006 && (wparam.0 & 0xffff) != 0 {
        if let Some(s) = state.as_mut() { SetFocus(s.h_out); }
    }
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
            if let Some(s) = state.as_mut() {
                match wparam.0 as i32 & 0xffff {
                    SIC_OK => {
                        let mut config = s.config.clone();
                        let out = get_text(s.h_out);
                        if !out.trim().is_empty() {
                            config.output_dir = PathBuf::from(out.trim());
                        }
                        config.default_segment_count = get_text(s.h_seg).parse().unwrap_or(8);
                        config.max_concurrent_downloads = get_text(s.h_conc).parse().unwrap_or(4);
                        config.max_retries = get_text(s.h_retry).parse().unwrap_or(3);
                        config.connect_timeout_secs = get_text(s.h_conn).parse().unwrap_or(30);
                        config.read_timeout_secs = get_text(s.h_read).parse().unwrap_or(60);
                        let mbps: f64 = get_text(s.h_speed).parse().unwrap_or(0.0);
                        config.max_speed_bps = if mbps > 0.0 {
                            Some((mbps * 1_048_576.0) as u64)
                        } else {
                            None
                        };
                        let proxy = get_text(s.h_proxy);
                        config.proxy_url = if proxy.trim().is_empty() { None } else { Some(proxy.trim().to_string()) };
                        config.watch_clipboard = SendMessageW(s.h_watch, 0x00F0, WPARAM(0), LPARAM(0)).0 != 0;
                        config.sched_enabled = SendMessageW(s.h_sched, 0x00F0, WPARAM(0), LPARAM(0)).0 != 0;
                        config.sched_start = get_text(s.h_start);
                        config.sched_stop = get_text(s.h_stop);
                        for control in &s.extras {
                            let value = get_text(control.hwnd);
                            let checked = if control.kind == ExtraKind::Bool {
                                SendMessageW(control.hwnd, 0x00F0, WPARAM(0), LPARAM(0)).0 != 0
                            } else {
                                false
                            };
                            apply_extra(&mut config, control.key, value, checked);
                        }
                        s.result = Some(config);
                        s.done = true;
                        DestroyWindow(hwnd);
                    }
                    SIC_CANCEL => {
                        s.done = true;
                        DestroyWindow(hwnd);
                    }
                    SIC_TEST_SOUND => {
                        if let Some(control) = s.extras.iter().find(|c| c.id == 3185) {
                            play_sound(&Some(get_text(control.hwnd)));
                        }
                    }
                    SIC_OUT_BROWSE => {
                        if let Some(dir) = browse_folder(GetParent(s.h_out_browse)) {
                            let mut b = wide(&dir);
                            let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(s.h_out, PCWSTR(b.as_mut_ptr()));
                        }
                    }
                    SIC_PROXY_IE => {
                        // Кнопка «Взять из IE» (мануал: Options.htm#Proxy).
                        let mut key = HKEY::default();
                        if RegOpenKeyExW(
                            HKEY_CURRENT_USER,
                            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings"),
                            0, KEY_READ, &mut key,
                        ).is_ok() {
                            let mut ty = REG_SZ;
                            let mut buf = [0u16; 512];
                            let mut len = (buf.len() * 2) as u32;
                            let ok = RegQueryValueExW(
                                key, w!("ProxyServer"), None, Some(&mut ty),
                                Some(buf.as_mut_ptr() as *mut u8), Some(&mut len),
                            ).is_ok();
                            RegCloseKey(key).ok();
                            if ok {
                                let server = String::from_utf16_lossy(&buf)
                                    .trim_end_matches('\0').to_string();
                                if !server.is_empty() {
                                    let mut b = wide(&format!("http://{}", server));
                                    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(s.h_proxy, PCWSTR(b.as_mut_ptr()));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_NOTIFY => {
            let nm = &*(lparam.0 as *const NMHDR);
            if nm.code == TCN_SELCHANGE {
                if let Some(s) = state.as_mut() {
                    let idx = SendMessageW(s.h_tab, TCM_GETCURSEL, WPARAM(0), LPARAM(0)).0 as usize;
                    for (i, page) in s.pages.iter().enumerate() {
                        ShowWindow(*page, if i == idx { SW_SHOW } else { windows::Win32::UI::WindowsAndMessaging::SW_HIDE });
                    }
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_CLOSE => {
            if let Some(s) = state.as_mut() {
                s.done = true;
            }
            DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn new_page(parent: HWND, id: i32) -> HWND {
    CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        w!("Static"),
        w!(""),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0),
        0, 34, 540, 470,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    )
}

unsafe fn setting_field(page: HWND, label: &str, id: i32, y: i32, value: &str, width: i32) -> HWND {
    create_label(page, label, 15, y, 160, 20);
    let h = create_edit(page, id, 175, y - 3, width, 22);
    let mut buf = wide(value);
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(h, PCWSTR(buf.as_mut_ptr()));
    h
}

fn bool_str(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

unsafe fn add_extra_multiline(
    state: &mut SettingsState,
    page: HWND,
    label: &str,
    id: i32,
    y: i32,
    value: &str,
    height: i32,
    key: ExtraKey,
) {
    create_label(page, label, 15, y, 160, 20);
    let h = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("Edit"),
        w!(""),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | 0x4 | 0x80 | 0x1000),
        175, y - 3, 360, height,
        page,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    );
    let mut buf = wide(value);
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(h, PCWSTR(buf.as_mut_ptr()));
    state.extras.push(ExtraControl { id, hwnd: h, key, kind: ExtraKind::Text });
}

unsafe fn add_extra(
    state: &mut SettingsState,
    page: HWND,
    label: &str,
    id: i32,
    y: i32,
    value: &str,
    width: i32,
    key: ExtraKey,
    kind: ExtraKind,
) {
    let hwnd = if kind == ExtraKind::Bool {
        setting_check(page, label, id, y, value == "1")
    } else {
        setting_field(page, label, id, y, value, width)
    };
    state.extras.push(ExtraControl { id, hwnd, key, kind });
}

fn split_lines(s: &str) -> Vec<String> {
    s.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

#[allow(dead_code)]
fn apply_extra(config: &mut DownloadConfig, key: ExtraKey, text: String, checked: bool) {
    let t = text.trim();
    match key {
        ExtraKey::StartWin => config.start_with_windows = checked,
        ExtraKey::BrowserIntegration => config.browser_integration = checked,
        ExtraKey::CtrlKey => config.capture_ctrl_key = checked,
        ExtraKey::AltKey => config.capture_alt_key = checked,
        ExtraKey::DetectApps => config.detect_new_apps = checked,
        ExtraKey::FileTypes => config.file_types = split_lines(&text),
        ExtraKey::ExcludedSites => config.excluded_sites = split_lines(&text),
        ExtraKey::ExcludedUrls => config.excluded_urls = split_lines(&text),
        ExtraKey::ConnType => config.connection_type = t.to_string(),
        ExtraKey::ConnExceptions => {
            config.connection_exceptions = split_lines(&text).into_iter().filter_map(|line| {
                let (site, count) = line.split_once(':')?;
                Some(crate::download::state::ConnectionException {
                    site: site.trim().to_string(),
                    connections: count.trim().parse().ok()?,
                })
            }).collect();
        }
        ExtraKey::LimitsEnabled => config.download_limits_enabled = checked,
        ExtraKey::LimitMb => config.download_limit_mb = t.parse().unwrap_or(0),
        ExtraKey::LimitHours => config.download_limit_hours = t.parse().unwrap_or(4),
        ExtraKey::CategoryFolders => {
            config.category_folders = split_lines(&text).into_iter().filter_map(|line| {
                let (category, path) = line.split_once('=')?;
                Some(crate::download::state::CategoryFolder {
                    category: category.trim().to_string(),
                    path: PathBuf::from(path.trim()),
                })
            }).collect();
        }
        ExtraKey::RememberPath => config.remember_last_path = checked,
        ExtraKey::ProgressDialog => config.progress_dialog = t.to_string(),
        ExtraKey::ShowStart => config.show_start_dialog = checked,
        ExtraKey::ShowComplete => config.show_complete_dialog = checked,
        ExtraKey::Silent => config.silent_mode = checked,
        ExtraKey::StartNow => config.start_immediately = checked,
        ExtraKey::IgnoreDate => config.ignore_date_on_resume = checked,
        ExtraKey::AvPath => config.antivirus_path = if t.is_empty() { None } else { Some(t.to_string()) },
        ExtraKey::AvParams => config.antivirus_params = t.to_string(),
        ExtraKey::ProxyUser => config.proxy_user = if t.is_empty() { None } else { Some(t.to_string()) },
        ExtraKey::ProxyPass => config.proxy_pass = if t.is_empty() { None } else { Some(t.to_string()) },
        ExtraKey::FtpPasv => config.ftp_pasv = checked,
        ExtraKey::UseIEProxy => config.use_ie_proxy = checked,
        ExtraKey::ProxyExclusions => config.proxy_exclusions = split_lines(&text),
        ExtraKey::SiteLogins => {
            config.site_logins = split_lines(&text).into_iter().filter_map(|line| {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() < 2 { return None; }
                Some(crate::download::state::SiteLogin {
                    url: parts[0].trim().to_string(),
                    username: parts.get(1).unwrap_or(&"").trim().to_string(),
                    password: parts.get(2).unwrap_or(&"").trim().to_string(),
                    cookie: parts.get(3).unwrap_or(&"").trim().to_string(),
                })
            }).collect();
        }
        ExtraKey::DialupConn => config.dialup_connection = t.to_string(),
        ExtraKey::DialupUser => config.dialup_user = t.to_string(),
        ExtraKey::DialupPass => config.dialup_pass = t.to_string(),
        ExtraKey::DialupSave => config.dialup_save_password = checked,
        ExtraKey::DialupAttempts => config.dialup_attempts = t.parse().unwrap_or(0),
        ExtraKey::DialupInterval => config.dialup_interval_secs = t.parse().unwrap_or(30),
        ExtraKey::SoundComplete => config.sound_complete = if t.is_empty() { None } else { Some(t.to_string()) },
        ExtraKey::SoundError => config.sound_error = if t.is_empty() { None } else { Some(t.to_string()) },
        ExtraKey::SoundStart => config.sound_start = if t.is_empty() { None } else { Some(t.to_string()) },
        ExtraKey::AutoNextQueue => config.auto_start_next_in_queue = checked,
        ExtraKey::ToolbarStyle => config.toolbar_style = t.parse().unwrap_or(0),
        ExtraKey::VisibleColumns => config.visible_columns = split_lines(&text),
        ExtraKey::BackupEnabled => config.backup_enabled = checked,
        ExtraKey::BackupDir => config.backup_dir = if t.is_empty() { None } else { Some(PathBuf::from(t)) },
        ExtraKey::ScanDownloads => config.scan_downloads = checked,
    }
}

unsafe fn setting_check(page: HWND, label: &str, id: i32, y: i32, checked: bool) -> HWND {
    let mut buf = wide(label);
    let h = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        w!("Button"),
        PCWSTR(buf.as_mut_ptr()),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | 0x2),
        15, y, 440, 22,
        page,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    );
    SendMessageW(h, 0x00F1, WPARAM(if checked { 1 } else { 0 }), LPARAM(0));
    h
}

fn show_settings_dialog(parent: HWND, config: &DownloadConfig) -> Option<DownloadConfig> {
    unsafe {
        let hinstance = GetModuleHandleW(None).ok()?;
        let mut class: WNDCLASSEXW = std::mem::zeroed();
        class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        class.lpfnWndProc = Some(settings_proc);
        class.hInstance = HINSTANCE(hinstance.0);
        class.hCursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        class.hbrBackground = HBRUSH((windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as isize);
        class.lpszClassName = w!("SDASettingsDialog");
        RegisterClassExW(&class);

        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0x2),
            w!("SDASettingsDialog"),
            w!("Настройки Internet Download Manager"),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !(0x20000 | 0x40000)),
            150, 120, 580, 540,
            parent,
            windows::Win32::UI::WindowsAndMessaging::HMENU(0),
            HINSTANCE(hinstance.0),
            None,
        );
        if hwnd.0 == 0 {
            return None;
        }

        let mut state = SettingsState {
            done: false,
            result: None,
            config: config.clone(),
            h_tab: HWND::default(),
            pages: Vec::new(),
            extras: Vec::new(),
            h_out: HWND::default(),
            h_out_browse: HWND::default(),
            h_seg: HWND::default(),
            h_conc: HWND::default(),
            h_watch: HWND::default(),
            h_speed: HWND::default(),
            h_retry: HWND::default(),
            h_conn: HWND::default(),
            h_read: HWND::default(),
            h_proxy: HWND::default(),
            h_sched: HWND::default(),
            h_start: HWND::default(),
            h_stop: HWND::default(),
        };

        state.h_tab = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            WC_TABCONTROL,
            w!(""),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | TCS_MULTILINE),
            5, 5, 560, 500,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(0),
            HINSTANCE(hinstance.0),
            None,
        );

        // Точная структура из idm_ru.lng / мануала: 9 вкладок в 2 ряда.
        let tabs = [
            "Основные",
            "Типы файлов",
            "Сохранить в",
            "Скачивание",
            "Соединение",
            "Прокси/Socks",
            "Пароли сайтов",
            "Автодозвон",
            "Звуки",
        ];
        for (i, tab) in tabs.iter().enumerate() {
            let mut buf = wide(tab);
            let mut item = TCITEMW {
                mask: TCIF_TEXT,
                pszText: windows::core::PWSTR(buf.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(state.h_tab, TCM_INSERTITEMW, WPARAM(i), LPARAM(&mut item as *mut _ as isize));
        }

        for i in 0..tabs.len() {
            state.pages.push(new_page(hwnd, 3200 + i as i32));
        }
        let pages = state.pages.clone();

        // Вкладка «Основные» (General)
        state.h_watch = setting_check(pages[0], "Следить за URL в буфере обмена", SIC_WATCH, 20,
            config.watch_clipboard);
        add_extra(&mut state, pages[0], "Запускать при входе в Windows", 3150, 80,
            bool_str(config.start_with_windows), 0, ExtraKey::StartWin, ExtraKind::Bool);
        add_extra(&mut state, pages[0], "Расширенная интеграция в браузеры", 3151, 110,
            bool_str(config.browser_integration), 0, ExtraKey::BrowserIntegration, ExtraKind::Bool);
        add_extra(&mut state, pages[0], "Ctrl + клик — принудительный перехват", 3152, 140,
            bool_str(config.capture_ctrl_key), 0, ExtraKey::CtrlKey, ExtraKind::Bool);
        add_extra(&mut state, pages[0], "Alt + клик — не перехватывать", 3153, 170,
            bool_str(config.capture_alt_key), 0, ExtraKey::AltKey, ExtraKind::Bool);
        add_extra(&mut state, pages[0], "Обнаруживать новые приложения", 3154, 200,
            bool_str(config.detect_new_apps), 0, ExtraKey::DetectApps, ExtraKind::Bool);

        // Вкладка «Типы файлов»
        create_label(pages[1], "Расширения — по одному в строке:", 15, 15, 500, 20);
        add_extra_multiline(&mut state, pages[1], "Типы файлов", 3155, 45,
            &config.file_types.join("\n"), 110, ExtraKey::FileTypes);
        add_extra_multiline(&mut state, pages[1], "Исключённые сайты (*.site.com)", 3156, 180,
            &config.excluded_sites.join("\n"), 70, ExtraKey::ExcludedSites);
        add_extra_multiline(&mut state, pages[1], "Исключённые URL", 3157, 275,
            &config.excluded_urls.join("\n"), 70, ExtraKey::ExcludedUrls);

        // Вкладка «Сохранить в» (Save To)
        state.h_out = setting_field(pages[2], "Папка загрузок:", SIC_OUT, 20,
            &config.output_dir.to_string_lossy(), 420);
        state.h_out_browse = create_button(pages[2], "...", SIC_OUT_BROWSE, 556, 17, 42, 22);
        create_label(pages[2], "Временные файлы сегментов: %TEMP%\\sda", 15, 50, 500, 20);
        add_extra_multiline(&mut state, pages[2], "Папки категорий (категория=путь)", 3163, 80,
            &config.category_folders.iter().map(|f| format!("{}={}", f.category, f.path.display())).collect::<Vec<_>>().join("\n"),
            130, ExtraKey::CategoryFolders);
        add_extra(&mut state, pages[2], "Запоминать последний путь категории", 3164, 240,
            bool_str(config.remember_last_path), 0, ExtraKey::RememberPath, ExtraKind::Bool);
        add_extra(&mut state, pages[2], "Резервная копия", 3188, 285,
            bool_str(config.backup_enabled), 0, ExtraKey::BackupEnabled, ExtraKind::Bool);
        let backup_dir = config.backup_dir.as_deref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        add_extra(&mut state, pages[2], "Папка резерва:", 3189, 320,
            &backup_dir, 360, ExtraKey::BackupDir, ExtraKind::Text);
        add_extra(&mut state, pages[2], "Сканировать загрузки", 3190, 355,
            bool_str(config.scan_downloads), 0, ExtraKey::ScanDownloads, ExtraKind::Bool);

        // Вкладка «Скачивание» (Downloads)
        state.h_sched = setting_check(pages[3], "Включить планировщик", SIC_SCHED, 20,
            config.sched_enabled);
        state.h_start = setting_field(pages[3], "Начало (ЧЧ:ММ):", SIC_START, 55,
            &config.sched_start, 80);
        state.h_stop = setting_field(pages[3], "Окончание (ЧЧ:ММ):", SIC_STOP, 90,
            &config.sched_stop, 80);
        add_extra(&mut state, pages[3], "Прогресс (show/minimize/hide)", 3165, 120,
            &config.progress_dialog, 120, ExtraKey::ProgressDialog, ExtraKind::Text);
        add_extra(&mut state, pages[3], "Показать диалог начала", 3166, 150,
            bool_str(config.show_start_dialog), 0, ExtraKey::ShowStart, ExtraKind::Bool);
        add_extra(&mut state, pages[3], "Показать диалог завершения", 3167, 180,
            bool_str(config.show_complete_dialog), 0, ExtraKey::ShowComplete, ExtraKind::Bool);
        add_extra(&mut state, pages[3], "Тихий режим", 3168, 210,
            bool_str(config.silent_mode), 0, ExtraKey::Silent, ExtraKind::Bool);
        add_extra(&mut state, pages[3], "Начинать сразу при показе диалога", 3169, 240,
            bool_str(config.start_immediately), 0, ExtraKey::StartNow, ExtraKind::Bool);
        add_extra(&mut state, pages[3], "Игнорировать смену даты при докачке", 3170, 270,
            bool_str(config.ignore_date_on_resume), 0, ExtraKey::IgnoreDate, ExtraKind::Bool);
        add_extra(&mut state, pages[3], "Антивирус (полный путь):", 3171, 300,
            config.antivirus_path.as_deref().unwrap_or(""), 330, ExtraKey::AvPath, ExtraKind::Text);
        add_extra(&mut state, pages[3], "Параметры антивируса:", 3172, 330,
            &config.antivirus_params, 330, ExtraKey::AvParams, ExtraKind::Text);
        add_extra(&mut state, pages[3], "Автозапуск следующей в очереди", 3191, 360,
            bool_str(config.auto_start_next_in_queue), 0, ExtraKey::AutoNextQueue, ExtraKind::Bool);
        add_extra(&mut state, pages[3], "Стиль панели (0-3):", 3192, 390,
            &config.toolbar_style.to_string(), 80, ExtraKey::ToolbarStyle, ExtraKind::Number);
        add_extra_multiline(&mut state, pages[3], "Видимые колонки", 3193, 420,
            &config.visible_columns.join("\n"), 45, ExtraKey::VisibleColumns);

        // Вкладка «Соединение» (Connection)
        state.h_seg = setting_field(pages[4], "Макс. количество потоков:", SIC_SEG, 20,
            &config.default_segment_count.to_string(), 60);
        state.h_conc = setting_field(pages[4], "Одновременных загрузок:", SIC_CONC, 55,
            &config.max_concurrent_downloads.to_string(), 60);
        state.h_speed = setting_field(pages[4], "Лимит скорости, МБ/с:", SIC_SPEED, 90,
            &config.max_speed_bps.map(|v| (v as f64 / 1_048_576.0).to_string()).unwrap_or_default(), 80);
        state.h_retry = setting_field(pages[4], "Повторов при ошибке:", SIC_RETRY, 125,
            &config.max_retries.to_string(), 60);
        state.h_conn = setting_field(pages[4], "Таймаут подключения, с:", SIC_CONN, 160,
            &config.connect_timeout_secs.to_string(), 60);
        state.h_read = setting_field(pages[4], "Таймаут чтения, с:", SIC_READ, 195,
            &config.read_timeout_secs.to_string(), 60);
        add_extra(&mut state, pages[4], "Тип соединения (high/medium/low)", 3158, 230,
            &config.connection_type, 100, ExtraKey::ConnType, ExtraKind::Text);
        add_extra(&mut state, pages[4], "Ограничения по загрузке", 3159, 260,
            bool_str(config.download_limits_enabled), 0, ExtraKey::LimitsEnabled, ExtraKind::Bool);
        add_extra(&mut state, pages[4], "Лимит, МБ за период:", 3160, 295,
            &config.download_limit_mb.to_string(), 80, ExtraKey::LimitMb, ExtraKind::Number);
        add_extra(&mut state, pages[4], "Период, часов:", 3161, 330,
            &config.download_limit_hours.to_string(), 80, ExtraKey::LimitHours, ExtraKind::Number);
        add_extra_multiline(&mut state, pages[4], "Исключения (сайт:потоки)", 3162, 365,
            &config.connection_exceptions.iter().map(|e| format!("{}:{}", e.site, e.connections)).collect::<Vec<_>>().join("\n"),
            90, ExtraKey::ConnExceptions);

        // Вкладка «Прокси/Socks»
        state.h_proxy = setting_field(pages[5], "Прокси URL (http://...):", SIC_PROXY, 20,
            config.proxy_url.as_deref().unwrap_or(""), 360);
        create_button(pages[5], "Взять из IE", SIC_PROXY_IE, 542, 17, 95, 22);
        add_extra(&mut state, pages[5], "Логин прокси:", 3173, 60,
            config.proxy_user.as_deref().unwrap_or(""), 200, ExtraKey::ProxyUser, ExtraKind::Text);
        add_extra(&mut state, pages[5], "Пароль прокси:", 3174, 95,
            config.proxy_pass.as_deref().unwrap_or(""), 200, ExtraKey::ProxyPass, ExtraKind::Text);
        add_extra(&mut state, pages[5], "FTP в режиме PASV", 3175, 130,
            bool_str(config.ftp_pasv), 0, ExtraKey::FtpPasv, ExtraKind::Bool);
        add_extra(&mut state, pages[5], "Использовать настройки IE", 3176, 160,
            bool_str(config.use_ie_proxy), 0, ExtraKey::UseIEProxy, ExtraKind::Bool);
        add_extra_multiline(&mut state, pages[5], "Исключения прокси", 3177, 195,
            &config.proxy_exclusions.join("\n"), 140, ExtraKey::ProxyExclusions);

        // Вкладка «Пароли сайтов»
        create_label(pages[6], "Формат строки: URL|логин|пароль|cookie", 15, 20, 500, 20);
        create_label(pages[6], "Применяется автоматически при добавлении задачи.", 15, 45, 500, 20);
        add_extra_multiline(&mut state, pages[6], "Сайты", 3178, 80,
            &config.site_logins.iter().map(|l| format!("{}|{}|{}|{}", l.url, l.username, l.password, l.cookie)).collect::<Vec<_>>().join("\n"),
            250, ExtraKey::SiteLogins);

        // Вкладка «Автодозвон» (Dial Up/VPN)
        add_extra(&mut state, pages[7], "Соединение:", 3179, 20,
            &config.dialup_connection, 300, ExtraKey::DialupConn, ExtraKind::Text);
        add_extra(&mut state, pages[7], "Логин:", 3180, 55,
            &config.dialup_user, 200, ExtraKey::DialupUser, ExtraKind::Text);
        add_extra(&mut state, pages[7], "Пароль:", 3181, 90,
            &config.dialup_pass, 200, ExtraKey::DialupPass, ExtraKind::Text);
        add_extra(&mut state, pages[7], "Сохранить пароль", 3182, 125,
            bool_str(config.dialup_save_password), 0, ExtraKey::DialupSave, ExtraKind::Bool);
        add_extra(&mut state, pages[7], "Попыток (0 — всегда):", 3183, 160,
            &config.dialup_attempts.to_string(), 80, ExtraKey::DialupAttempts, ExtraKind::Number);
        add_extra(&mut state, pages[7], "Интервал, с:", 3184, 195,
            &config.dialup_interval_secs.to_string(), 80, ExtraKey::DialupInterval, ExtraKind::Number);

        // Вкладка «Звуки»
        add_extra(&mut state, pages[8], "Завершена:", 3185, 20,
            config.sound_complete.as_deref().unwrap_or(""), 330, ExtraKey::SoundComplete, ExtraKind::Text);
        add_extra(&mut state, pages[8], "Ошибка:", 3186, 55,
            config.sound_error.as_deref().unwrap_or(""), 330, ExtraKey::SoundError, ExtraKind::Text);
        add_extra(&mut state, pages[8], "Начало:", 3187, 90,
            config.sound_start.as_deref().unwrap_or(""), 330, ExtraKey::SoundStart, ExtraKind::Text);
        create_button(pages[8], "Тест", SIC_TEST_SOUND, 15, 130, 90, 26);

        for (i, page) in state.pages.iter().enumerate() {
            ShowWindow(*page, if i == 0 { SW_SHOW } else { windows::Win32::UI::WindowsAndMessaging::SW_HIDE });
        }

        create_button(hwnd, "ОК", SIC_OK, 350, 495, 90, 28);
        create_button(hwnd, "Отмена", SIC_CANCEL, 460, 495, 100, 28);

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut state as *mut _ as isize);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        EnableWindow(parent, FALSE);
        SetForegroundWindow(hwnd);
        SetFocus(state.h_out);
        let mut msg = std::mem::zeroed();
        while !state.done && GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
            if !modal_key(hwnd, &msg, false) {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        EnableWindow(parent, TRUE);
        SetForegroundWindow(parent);
        state.result
    }
}

// ---------------------------------------------------------------------------
// Главное окно
// ---------------------------------------------------------------------------

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_NCCREATE {
        let cs = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let state = ptr.as_mut();

    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_CREATE => {
            if let Some(s) = state {
                s.hwnd = hwnd;
                s.h_tree = create_control(hwnd, WC_TREEVIEW, ID_TREE, WS_BORDER.0 | 0x2 | 0x4 | 0x1);
                s.h_list = create_control(hwnd, WC_LISTVIEW, ID_LIST, WS_BORDER.0 | LVS_REPORT | LVS_SINGLESEL);
                s.h_status = create_control(hwnd, w!("msctls_statusbar32"), ID_STATUS, 0);
                s.h_toolbar = create_toolbar(hwnd);
                create_menu(hwnd);
                SendMessageW(
                    s.h_list,
                    LVM_SETEXTENDEDLISTVIEWSTYLE,
                    WPARAM(0),
                    LPARAM((LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES) as isize),
                );
                let mut col_index = 0usize;
                for (name, width) in COLUMNS {
                    if s.config.visible_columns.iter().any(|c| c.eq_ignore_ascii_case(name)) {
                        insert_column(s.h_list, name, width, col_index);
                        col_index += 1;
                    }
                }
                let root = insert_tree_item(s.h_tree, CATEGORIES[0].0, TVI_ROOT);
                for (cat, is_root) in CATEGORIES.iter().skip(1) {
                    insert_tree_item(s.h_tree, cat, if *is_root { TVI_ROOT } else { root });
                }
                let parts = [400, 900];
                SendMessageW(
                    s.h_status,
                    SB_SETPARTS,
                    WPARAM(parts.len()),
                    LPARAM(parts.as_ptr() as isize),
                );
                SetTimer(hwnd, ID_TIMER, 500, None);
                unsafe { tray_add(hwnd) };
                s.refresh();
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_SIZE => {
            if let Some(s) = state {
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect);
                let w = rect.right;
                let h = rect.bottom;
                windows::Win32::UI::WindowsAndMessaging::MoveWindow(s.h_toolbar, 0, 0, w, 56, true);
                SendMessageW(s.h_toolbar, TB_AUTOSIZE, WPARAM(0), LPARAM(0));
                let top = 58;
                let bottom = h - 24;
                windows::Win32::UI::WindowsAndMessaging::MoveWindow(s.h_tree, 0, top, 205, bottom - top, true);
                windows::Win32::UI::WindowsAndMessaging::MoveWindow(s.h_list, 207, top, w - 207, bottom - top, true);
                windows::Win32::UI::WindowsAndMessaging::MoveWindow(s.h_status, 0, bottom, w, 24, true);
                let parts = [w / 2, w];
                SendMessageW(s.h_status, SB_SETPARTS, WPARAM(2), LPARAM(parts.as_ptr() as isize));
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_TIMER => {
            if wparam.0 == ID_TIMER {
                if let Some(s) = state {
                    s.on_tick();
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN => {
            // Горячие клавиши IDM: Ctrl+F — найти, F3 — найти следующий.
            if let Some(s) = state {
                let vk = wparam.0 as i32;
                let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0;
                if ctrl && vk == 0x46 {
                    s.handle_command(CMD_FIND);
                    return LRESULT(0);
                }
                if vk == 0x7A {
                    s.handle_command(CMD_FIND_NEXT);
                    return LRESULT(0);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
            if let Some(s) = state {
                s.handle_command(wparam.0 & 0xffff);
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_CONTEXTMENU => {
            if let Some(s) = state {
                let source = HWND(wparam.0 as isize);
                if source == s.h_list || source == hwnd {
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    unsafe {
                        let menu = CreatePopupMenu().unwrap_or_default();
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_START, w!("Старт"));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_PAUSE, w!("Пауза"));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_RESUME, w!("Возобновить"));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_CANCEL, w!("Отмена"));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_REMOVE, w!("Удалить"));
                        let _ = AppendMenuW(menu, MF_STRING, 0, w!(""));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_PROPERTIES, w!("Свойства"));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_OPEN_FOLDER, w!("Открыть папку"));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_CTX_COPY_URL, w!("Копировать URL"));
                        TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, None);
                    }
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_NOTIFY => {
            if let Some(s) = state {
                s.on_notify(lparam);
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            KillTimer(hwnd, ID_TIMER);
            unsafe { tray_remove(hwnd) };
            PostQuitMessage(0);
            LRESULT(0)
        }
        msg if msg == WM_APP + 1 => {
            match lparam.0 as u32 {
                0x0203 => { // WM_LBUTTONDBLCLK
                    ShowWindow(hwnd, SW_SHOW);
                }
                0x0205 => { // WM_RBUTTONUP — меню трея (мануал: Выход)
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    unsafe {
                        let menu = CreatePopupMenu().unwrap_or_default();
                        let _ = AppendMenuW(menu, MF_STRING, CMD_TRAY_SHOW, w!("Показать окно"));
                        let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT, w!("Выход"));
                        SetForegroundWindow(hwnd);
                        TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, None);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn create_control(hwnd: HWND, class: windows::core::PCWSTR, id: i32, style: u32) -> HWND {
    CreateWindowExW(
        WS_EX_CLIENTEDGE,
        class,
        w!(""),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
        0,
        0,
        100,
        100,
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    )
}

unsafe fn insert_column(h: HWND, name: &str, width: i32, index: usize) {
    let mut buf = wide(name);
    let mut col = LVCOLUMNW {
        mask: LVCF_FMT | LVCF_WIDTH | LVCF_TEXT | LVCF_SUBITEM,
        fmt: LVCFMT_LEFT,
        cx: width,
        pszText: PWSTR(buf.as_mut_ptr()),
        ..Default::default()
    };
    SendMessageW(h, LVM_INSERTCOLUMNW, WPARAM(index), LPARAM(&mut col as *mut _ as isize));
}

unsafe fn insert_tree_item(
    h: HWND,
    text: &str,
    parent: windows::Win32::UI::Controls::HTREEITEM,
) -> windows::Win32::UI::Controls::HTREEITEM {
    let mut buf = wide(text);
    let item = TVITEMW {
        mask: TVIF_TEXT,
        pszText: PWSTR(buf.as_mut_ptr()),
        ..Default::default()
    };
    let mut tvis = TVINSERTSTRUCTW {
        hParent: parent,
        hInsertAfter: TVI_LAST,
        Anonymous: TVINSERTSTRUCTW_0 { item },
    };
    let result = SendMessageW(h, TVM_INSERTITEMW, WPARAM(0), LPARAM(&mut tvis as *mut _ as isize));
    windows::Win32::UI::Controls::HTREEITEM(result.0)
}

// ---------------------------------------------------------------------------
// Диалог 103 «Прогресс загрузки» (Download progress)
// ---------------------------------------------------------------------------

struct ProgressState {
    hwnd: HWND,
    app: *mut AppState,
    job_id: uuid::Uuid,
    h_tab: HWND,
    h_info: HWND,
    h_progress: HWND,
    h_list: HWND,
    h_details: HWND,
    h_pause: HWND,
    h_speed_limit: HWND,
    pages: Vec<HWND>,
    details_visible: bool,
    paused: bool,
}

unsafe fn update_progress(state: &mut ProgressState) {
    let app = &mut *state.app;
    let Some(job) = app.runtime.block_on(app.manager.get_job(state.job_id)) else {
        return;
    };
    let pct = if job.total_bytes > 0 {
        ((job.downloaded_bytes as f64 / job.total_bytes as f64) * 100.0).min(100.0) as u32
    } else {
        0
    };
    let name = PathBuf::from(&job.path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut title = wide(&format!("{}% {}", pct, name));
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(state.hwnd, PCWSTR(title.as_mut_ptr()));
    SendMessageW(state.h_progress, PBM_SETPOS, WPARAM(pct as usize), LPARAM(0));

    let info = format!(
        "Статус: {}\nРазмер файла: {}\nЗагружено: {} ({:.2}%)\nСкорость передачи: {}\nОсталось времени: {}\nПоддержка докачки: Да",
        fmt_status(&job),
        fmt_size(job.total_bytes),
        fmt_size(job.downloaded_bytes),
        if job.total_bytes > 0 { job.downloaded_bytes as f64 / job.total_bytes as f64 * 100.0 } else { 0.0 },
        fmt_speed(job.speed_bps),
        fmt_left(&job),
    );
    let mut buf = wide(&info);
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(state.h_info, PCWSTR(buf.as_mut_ptr()));

    // Список соединений: сводная строка по сегментам задачи.
    SendMessageW(state.h_list, LVM_DELETEALLITEMS, WPARAM(0), LPARAM(0));
    let mut name_buf = wide(&name);
    let mut item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: 0,
        iSubItem: 0,
        pszText: PWSTR(name_buf.as_mut_ptr()),
        lParam: LPARAM(0),
        ..Default::default()
    };
    SendMessageW(state.h_list, LVM_INSERTITEMW, WPARAM(0), LPARAM(&mut item as *mut _ as isize));
    set_list_text(state.h_list, 0, 1, &fmt_size(job.downloaded_bytes));
    set_list_text(state.h_list, 0, 2, &fmt_status(&job));

    let label = if state.paused { w!("Старт") } else { w!("Пауза") };
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(state.h_pause, label);
}

unsafe extern "system" fn progress_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_NCCREATE {
        let cs = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProgressState;
    if msg == 0x0028 {
        return handle_next_dlgctl(hwnd, wparam, lparam);
    }
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_CREATE => {
            if let Some(s) = state.as_mut() {
                s.hwnd = hwnd;
                s.h_tab = CreateWindowExW(
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
                    WC_TABCONTROL, w!(""),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
                    19, 10, 503, 26, hwnd,
                    windows::Win32::UI::WindowsAndMessaging::HMENU(0),
                    HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0), None,
                );
                for (i, tab) in ["Состояние скачивания", "Ограничитель скорости", "Опции при завершении"].into_iter().enumerate() {
                    let mut buf = wide(tab);
                    let mut item = TCITEMW { mask: TCIF_TEXT, pszText: windows::core::PWSTR(buf.as_mut_ptr()), ..Default::default() };
                    SendMessageW(s.h_tab, TCM_INSERTITEMW, WPARAM(i), LPARAM(&mut item as *mut _ as isize));
                }
                for i in 0..3 {
                    s.pages.push(new_page(hwnd, 3800 + i as i32));
                }
                let pages = s.pages.clone();
                s.h_info = create_label(pages[0], "", 19, 39, 503, 164);
                s.h_speed_limit = create_label(pages[1], "Ограничитель скорости: настройте на вкладке «Соединение» в опциях.", 19, 39, 503, 80);
                create_label(pages[2], "Опции при завершении", 19, 39, 503, 30);
                for (i, page) in pages.iter().enumerate() {
                    ShowWindow(*page, if i == 0 { SW_SHOW } else { windows::Win32::UI::WindowsAndMessaging::SW_HIDE });
                }
                // Точные координаты диалога 103.
                s.h_progress = CreateWindowExW(
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
                    w!("msctls_progress32"), w!(""),
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
                    19, 211, 503, 16, hwnd,
                    windows::Win32::UI::WindowsAndMessaging::HMENU(1146),
                    HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0), None,
                );
                s.h_details = create_button(hwnd, "<< Скрыть детали", 1178, 31, 236, 140, 24);
                s.h_pause = create_button(hwnd, "Пауза", 1158, 313, 236, 84, 24);
                create_button(hwnd, "Отмена", 2, 424, 236, 75, 24);
                create_label(hwnd, "Стартовые позиции и прогресс скачивания по соединениям", 40, 263, 459, 15);
                s.h_list = create_control(hwnd, WC_LISTVIEW, 1177, WS_BORDER.0 | LVS_REPORT | LVS_SINGLESEL);
                windows::Win32::UI::WindowsAndMessaging::MoveWindow(s.h_list, 19, 304, 503, 128, true);
                for (i, (name, width)) in [("Загружено", 220), ("Информация", 280)].into_iter().enumerate() {
                    insert_column(s.h_list, name, width, i);
                }
                SetTimer(hwnd, 1, 500, None);
                update_progress(s);
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_TIMER => {
            if let Some(s) = state.as_mut() { update_progress(s); }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
            if let Some(s) = state.as_mut() {
                match wparam.0 as i32 & 0xffff {
                    1178 => {
                        s.details_visible = !s.details_visible;
                        let show = if s.details_visible { SW_SHOW } else { windows::Win32::UI::WindowsAndMessaging::SW_HIDE };
                        ShowWindow(s.h_list, show);
                        let label = if s.details_visible { w!("<< Скрыть детали") } else { w!("Показать детали >>") };
                        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(s.h_details, label);
                    }
                    1158 => {
                        let app = &mut *s.app;
                        if s.paused {
                            let _ = app.runtime.block_on(app.manager.resume_job(s.job_id));
                        } else {
                            let _ = app.runtime.block_on(app.manager.pause_job(s.job_id));
                        }
                        s.paused = !s.paused;
                        update_progress(s);
                    }
                    2 => { DestroyWindow(hwnd); }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_NOTIFY => {
            let nm = &*(lparam.0 as *const NMHDR);
            if nm.code == TCN_SELCHANGE {
                if let Some(s) = state.as_mut() {
                    let idx = SendMessageW(s.h_tab, TCM_GETCURSEL, WPARAM(0), LPARAM(0)).0 as usize;
                    for (i, page) in s.pages.iter().enumerate() {
                        ShowWindow(*page, if i == idx { SW_SHOW } else { windows::Win32::UI::WindowsAndMessaging::SW_HIDE });
                    }
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            KillTimer(hwnd, 1);
            if !state.is_null() { drop(Box::from_raw(state)); }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn open_progress_dialog(parent: HWND, app: *mut AppState, job_id: uuid::Uuid) {
    static mut REGISTERED: bool = false;
    if !REGISTERED {
        let mut class: WNDCLASSEXW = std::mem::zeroed();
        class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        class.lpfnWndProc = Some(progress_proc);
        class.hInstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        class.hCursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        class.hbrBackground = HBRUSH((windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as isize);
        class.lpszClassName = w!("SDAProgressDlg");
        RegisterClassExW(&class);
        REGISTERED = true;
    }
    let state = Box::into_raw(Box::new(ProgressState {
        hwnd: HWND::default(),
        app,
        job_id,
        h_tab: HWND::default(),
        h_info: HWND::default(),
        h_progress: HWND::default(),
        h_list: HWND::default(),
        h_details: HWND::default(),
        h_pause: HWND::default(),
        h_speed_limit: HWND::default(),
        pages: Vec::new(),
        details_visible: true,
        paused: false,
    }));
    let hwnd = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        w!("SDAProgressDlg"), w!("0%"),
        WS_OVERLAPPEDWINDOW,
        300, 200, 543, 445, parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(0),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        Some(state as *const c_void),
    );
    if hwnd.0 != 0 {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    } else {
        drop(Box::from_raw(state));
    }
}

unsafe fn show_properties(parent: HWND, job: &JobInfo) {
    struct PropState { done: bool, opened: bool, h_save: HWND }
    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PropState;
        if msg == 0x0028 {
            return handle_next_dlgctl(hwnd, wparam, lparam);
        }
        if msg == 0x0400 {
            return LRESULT(((DC_HASDEFID as isize) << 16) | 1092);
        }
        if msg == 0x0006 && (wparam.0 & 0xffff) != 0 {
            if let Some(s) = state.as_mut() { SetFocus(s.h_save); }
        }
        match msg {
            windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
                match wparam.0 as i32 & 0xffff {
                    1164 => {
                        if let Some(s) = state.as_mut() { s.done = true; s.opened = true; }
                        DestroyWindow(hwnd);
                    }
                    1092 | 2 => {
                        if let Some(s) = state.as_mut() { s.done = true; }
                        DestroyWindow(hwnd);
                    }
                    1082 => {
                        if let Some(s) = state.as_mut() {
                            if let Some(dir) = browse_folder(GetParent(hwnd)) {
                                let mut b = wide(&dir);
                                let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(s.h_save, PCWSTR(b.as_mut_ptr()));
                            }
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            windows::Win32::UI::WindowsAndMessaging::WM_CLOSE => {
                if let Some(s) = state.as_mut() { s.done = true; }
                DestroyWindow(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    let hinstance = match GetModuleHandleW(None) { Ok(h) => h, Err(_) => return };
    let mut class: WNDCLASSEXW = std::mem::zeroed();
    class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
    class.lpfnWndProc = Some(proc);
    class.hInstance = HINSTANCE(hinstance.0);
    class.hCursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
    class.hbrBackground = HBRUSH((windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as isize);
    class.lpszClassName = w!("SDAFileProperties");
    RegisterClassExW(&class);

    let hwnd = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0x2),
        w!("SDAFileProperties"),
        w!("Свойства файла"),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !(0x20000 | 0x40000)),
        260, 180, 493, 439,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(0),
        HINSTANCE(hinstance.0),
        None,
    );
    if hwnd.0 == 0 { return; }

    create_label(hwnd, "Имя файла:", 19, 42, 70, 16);
    create_label(hwnd, &PathBuf::from(&job.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), 95, 52, 380, 15);
    create_label(hwnd, "Тип:", 19, 90, 66, 15);
    create_label(hwnd, PathBuf::from(&job.path).extension().and_then(|e| e.to_str()).unwrap_or("файл"), 95, 90, 380, 15);
    create_label(hwnd, "Статус:", 19, 109, 65, 15);
    create_label(hwnd, &fmt_status(job), 95, 109, 380, 15);
    create_label(hwnd, "Размер:", 19, 129, 65, 15);
    create_label(hwnd, &fmt_size(job.total_bytes), 95, 129, 380, 15);
    create_label(hwnd, "Сохранить в:", 19, 156, 68, 15);
    let save_path = job.path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let h_save = create_edit(hwnd, 1096, 95, 151, 296, 23);
    let mut buf = wide(&save_path);
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(h_save, PCWSTR(buf.as_mut_ptr()));
    create_button(hwnd, "Обзор...", 1082, 400, 151, 75, 23);
    create_label(hwnd, "Адрес:", 19, 187, 66, 15);
    let h_url = create_edit(hwnd, 1097, 95, 182, 380, 23);
    let mut buf = wide(&job.url);
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(h_url, PCWSTR(buf.as_mut_ptr()));
    create_label(hwnd, "Описание:", 19, 218, 71, 15);
    create_edit(hwnd, 1099, 95, 213, 380, 23);
    create_label(hwnd, "Веб-страница, с которой получен файл:", 19, 252, 456, 15);
    create_label(hwnd, "Неизвестно", 19, 273, 456, 15);
    create_label(hwnd, "Referer:", 19, 309, 71, 15);
    create_edit(hwnd, 1569, 95, 302, 380, 23);
    create_label(hwnd, "Логин", 43, 337, 75, 15);
    create_edit(hwnd, 1033, 128, 332, 189, 21);
    create_label(hwnd, "Пароль", 40, 364, 78, 15);
    create_edit(hwnd, 1041, 128, 361, 189, 21);
    create_label(hwnd, "Дата последней попытки", 19, 397, 456, 15);
    create_label(hwnd, "Результат: ", 19, 416, 456, 13);
    create_button(hwnd, "Открыть", 1164, 266, 400, 75, 23);
    create_button(hwnd, "OK", 1092, 400, 400, 75, 23);

    let mut state = PropState { done: false, opened: false, h_save };
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut state as *mut _ as isize);
    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    EnableWindow(parent, FALSE);
    SetForegroundWindow(hwnd);
    SetFocus(h_save);
    let mut msg = std::mem::zeroed();
    while !state.done && GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
        if !modal_key(hwnd, &msg, false) {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    EnableWindow(parent, TRUE);
    SetForegroundWindow(parent);
    if state.opened {
        open_path(&job.path);
    }
}

unsafe fn load_icon_bitmap(name: &str) -> Option<windows::Win32::Graphics::Gdi::HBITMAP> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let file = format!("{}.svg", name);
    let mut path = exe_dir.join("resources").join("icons").join(&file);
    if !path.is_file() {
        path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources").join("icons").join(&file);
    }
    let svg = std::fs::read_to_string(path).ok()?;
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).ok()?;
    let size = tree.size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(32, 32)?;
    let transform = resvg::tiny_skia::Transform::from_scale(
        32.0 / size.width().max(1.0),
        32.0 / size.height().max(1.0),
    );
    let mut data = pixmap.data_mut();
    let mut pm = resvg::tiny_skia::PixmapMut::from_bytes(&mut data, 32, 32)?;
    resvg::render(&tree, transform, &mut pm);
    let src = pm.as_ref().data();

    // resvg выдаёт premultiplied RGBA. Композитируем на белый фон и делаем
    // белый цвет маской прозрачности ImageList (иначе иконки выглядят чёрными).
    let mut bgra = vec![0u8; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let s = ((31 - y) * 32 + x) * 4;
            let d = (y * 32 + x) * 4;
            let a = src[s + 3] as u16;
            let r = (src[s] as u16 + 255 - a).min(255) as u8;
            let g = (src[s + 1] as u16 + 255 - a).min(255) as u8;
            let b = (src[s + 2] as u16 + 255 - a).min(255) as u8;
            bgra[d] = b;
            bgra[d + 1] = g;
            bgra[d + 2] = r;
            bgra[d + 3] = 255;
        }
    }
    Some(CreateBitmap(32, 32, 1, 32, Some(bgra.as_ptr() as *const c_void)))
}

unsafe fn create_toolbar(parent: HWND) -> HWND {
    let h = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        w!("ToolbarWindow32"),
        w!(""),
        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | TBSTYLE_FLAT),
        0, 0, 100, 56, parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(ID_TOOLBAR as isize),
        HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
        None,
    );

    let icon_names = [
        "download", "start", "stop", "stop_all", "stop", "refresh",
        "settings", "schedule", "launch", "stop", "grabber", "launch",
    ];
    let himl = ImageList_Create(32, 32, ILC_MASK | ILC_COLOR32, 12, 1);
    let mut icon_indices: Vec<i32> = Vec::new();
    for name in icon_names {
        let mut index = 0;
        if let Some(bitmap) = load_icon_bitmap(name) {
            index = ImageList_AddMasked(himl, bitmap, COLORREF(0x00FFFFFF));
        }
        icon_indices.push(index);
    }
    SendMessageW(h, TB_SETIMAGELIST, WPARAM(0), LPARAM(himl.0 as isize));

    let buttons: [(usize, &str); 13] = [
        (CMD_ADD, "Добавить"),
        (CMD_RESUME, "Возобновить"),
        (CMD_STOP, "Стоп"),
        (CMD_STOP_ALL, "Стоп все"),
        (usize::MAX, ""), // разделитель
        (CMD_REMOVE, "Удалить"),
        (CMD_CLEAR, "Очистить"),
        (CMD_SETTINGS, "Настройки"),
        (CMD_SCHEDULE, "Расписание"),
        (CMD_START_QUEUE, "Запустить очередь"),
        (CMD_STOP_QUEUE, "Остановить очередь"),
        (CMD_GRABBER, "Граббер"),
        (CMD_TELL_FRIEND, "Рассказать"),
    ];
    for (i, (id, text)) in buttons.into_iter().enumerate() {
        if id == usize::MAX {
            let sep = TBBUTTON {
                iBitmap: 0,
                idCommand: 0,
                fsState: 0,
                fsStyle: 0x1, // BTNS_SEP
                bReserved: [0; 6],
                dwData: 0,
                iString: -1,
            };
            SendMessageW(h, TB_INSERTBUTTONW, WPARAM(i), LPARAM(&sep as *const _ as isize));
            continue;
        }
        let mut buf = wide(text);
        let idx = SendMessageW(h, TB_ADDSTRINGW, WPARAM(0), LPARAM(buf.as_mut_ptr() as isize)).0 as isize;
        let btn = TBBUTTON {
            iBitmap: (i.min(icon_indices.len() - 1)) as i32,
            idCommand: id as i32,
            fsState: TBSTATE_ENABLED as u8,
            fsStyle: 0x40, // BTNS_SHOWTEXT — иконка + подпись, как в IDM
            bReserved: [0; 6],
            dwData: 0,
            iString: idx,
        };
        SendMessageW(h, TB_INSERTBUTTONW, WPARAM(i), LPARAM(&btn as *const _ as isize));
    }
    SendMessageW(h, TB_AUTOSIZE, WPARAM(0), LPARAM(0));
    h
}

unsafe fn create_menu(hwnd: HWND) {
    let menu = CreateMenu().unwrap_or_default();

    // Задачи (m32771)
    let tasks = CreateMenu().unwrap_or_default();
    let _ = AppendMenuW(tasks, MF_STRING, CMD_ADD, w!("Добавить новую загрузку"));
    let _ = AppendMenuW(tasks, MF_STRING, CMD_ADD_BATCH, w!("Добавить пакетную загрузку"));
    let _ = AppendMenuW(tasks, MF_STRING, CMD_ADD_BATCH_CLIP, w!("Добавить пакетную загрузку из буфера обмена"));
    let _ = AppendMenuW(tasks, MF_STRING, CMD_GRABBER_SITE, w!("Запустить граббер сайта"));
    let _ = AppendMenuW(tasks, MF_STRING, CMD_DROP_TARGET, w!("Показывать приёмник для перетаскивания"));
    let _ = AppendMenuW(tasks, MF_STRING, 0, w!(""));
    let _ = AppendMenuW(tasks, MF_STRING, CMD_EXPORT, w!("Экспорт"));
    let _ = AppendMenuW(tasks, MF_STRING, CMD_IMPORT, w!("Импорт"));
    let _ = AppendMenuW(tasks, MF_STRING, 0, w!(""));
    let _ = AppendMenuW(tasks, MF_STRING, CMD_EXIT, w!("Выход"));

    // Файл (m32774)
    let file = CreateMenu().unwrap_or_default();
    let _ = AppendMenuW(file, MF_STRING, CMD_STOP, w!("Остановить скачивание"));
    let _ = AppendMenuW(file, MF_STRING, CMD_REMOVE, w!("Удалить"));
    let _ = AppendMenuW(file, MF_STRING, CMD_START, w!("Начать скачивание"));
    let _ = AppendMenuW(file, MF_STRING, CMD_REDOWNLOAD, w!("Перекачать"));
    let _ = AppendMenuW(file, MF_STRING, CMD_MOVE_RENAME, w!("Переместить/Переименовать"));

    // Скачивание (m32778)
    let download = CreateMenu().unwrap_or_default();
    let _ = AppendMenuW(download, MF_STRING, CMD_PAUSE_ALL, w!("Пауза для всех"));
    let _ = AppendMenuW(download, MF_STRING, CMD_STOP_ALL, w!("Остановить все"));
    let _ = AppendMenuW(download, MF_STRING, CMD_DELETE_COMPLETED, w!("Удалить все завершённые"));
    let _ = AppendMenuW(download, MF_STRING, 0, w!(""));
    let _ = AppendMenuW(download, MF_STRING, CMD_START_QUEUE, w!("Запустить очередь"));
    let _ = AppendMenuW(download, MF_STRING, CMD_STOP_QUEUE, w!("Остановить очередь"));
    let _ = AppendMenuW(download, MF_STRING, CMD_SPEED_LIMIT, w!("Ограничитель скорости"));
    let _ = AppendMenuW(download, MF_STRING, CMD_SCHEDULE, w!("Планировщик"));
    let _ = AppendMenuW(download, MF_STRING, CMD_SETTINGS, w!("Настройки"));
    let _ = AppendMenuW(download, MF_STRING, 0, w!(""));
    let _ = AppendMenuW(download, MF_STRING, CMD_FIND, w!("Найти\tCtrl+F"));
    let _ = AppendMenuW(download, MF_STRING, CMD_FIND_NEXT, w!("Найти Следующий\tF3"));

    // Вид (m32825)
    let view = CreateMenu().unwrap_or_default();
    let _ = AppendMenuW(view, MF_STRING, CMD_HIDE_CATEGORIES, w!("Скрыть категории"));
    let arrange = CreateMenu().unwrap_or_default();
    for (i, label) in ["По порядку добавления", "По имени файла", "По размеру", "По состоянию",
        "По оставшемуся времени", "По скорости", "По описанию", "По пути сохранения",
        "По Referer", "По дате последней попытки"].into_iter().enumerate() {
        let _ = AppendMenuW(arrange, MF_STRING, CMD_SORT_BASE + i, PCWSTR(wide(label).as_ptr()));
    }
    let _ = AppendMenuW(view, MF_STRING | MF_POPUP, arrange.0 as usize, w!("Упорядочить файлы"));
    let _ = AppendMenuW(view, MF_STRING, CMD_TOOLBAR, w!("Панель инструментов"));
    let _ = AppendMenuW(view, MF_STRING, CMD_COLUMNS, w!("Столбцы"));
    let _ = AppendMenuW(view, MF_STRING, CMD_LANGUAGE, w!("Язык"));

    // Помощь (m57670)
    let help = CreateMenu().unwrap_or_default();
    let _ = AppendMenuW(help, MF_STRING, CMD_TUTORIALS, w!("Обучение"));
    let _ = AppendMenuW(help, MF_STRING, CMD_TIP, w!("Совет дня"));
    let _ = AppendMenuW(help, MF_STRING, CMD_HOMEPAGE, w!("Сайт программы IDM"));
    let _ = AppendMenuW(help, MF_STRING, CMD_SUPPORT, w!("Обратиться в службу поддержки IDM"));
    let _ = AppendMenuW(help, MF_STRING, CMD_UPDATE, w!("Проверить наличие новой версии"));
    let _ = AppendMenuW(help, MF_STRING, CMD_ABOUT, w!("О программе IDM"));
    let _ = AppendMenuW(help, MF_STRING, CMD_TELL_FRIEND, w!("Письмо другу"));

    // Регистрация (m32785)
    let reg = CreateMenu().unwrap_or_default();
    let _ = AppendMenuW(reg, MF_STRING, CMD_ORDER, w!("Заказ онлайн"));
    let _ = AppendMenuW(reg, MF_STRING, CMD_REGISTRATION, w!("Регистрация"));

    let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, tasks.0 as usize, w!("Задачи"));
    let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, file.0 as usize, w!("Файл"));
    let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, download.0 as usize, w!("Скачивание"));
    let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, view.0 as usize, w!("Вид"));
    let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, help.0 as usize, w!("Помощь"));
    let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, reg.0 as usize, w!("Регистрация"));

    SetMenu(hwnd, menu).ok();
    DrawMenuBar(hwnd).ok();
}

/// Запуск нативного GUI.
pub fn run() -> anyhow::Result<()> {
    unsafe {
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_TREEVIEW_CLASSES | ICC_BAR_CLASSES,
        });

        let hinstance = GetModuleHandleW(None)?;
        let icon = LoadIconW(HINSTANCE::default(), IDI_APPLICATION).unwrap_or_default();
        let mut class: WNDCLASSEXW = std::mem::zeroed();
        class.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        class.style = WNDCLASS_STYLES(2 | 8); // CS_HREDRAW | CS_VREDRAW
        class.lpfnWndProc = Some(wnd_proc);
        class.hInstance = hinstance.into();
        class.hIcon = icon;
        class.hCursor = LoadCursorW(HINSTANCE::default(), IDC_ARROW).unwrap_or_default();
        class.hbrBackground = HBRUSH((windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as isize);
        class.lpszClassName = w!("SDAWin32Main");
        class.hIconSm = icon;
        RegisterClassExW(&class);

        let mut state = AppState::new()?;
        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            w!("SDAWin32Main"),
            w!("Internet Download Manager 6.32"),
            WS_OVERLAPPEDWINDOW,
            100,
            100,
            1400,
            780,
            HWND::default(),
            windows::Win32::UI::WindowsAndMessaging::HMENU(0),
            HINSTANCE(hinstance.0),
            Some(&mut state as *mut _ as *const c_void),
        );
        if hwnd.0 == 0 {
            anyhow::bail!("Не удалось создать главное окно");
        }

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
            // Акселераторы IDM перехватываем в цикле: WM_KEYDOWN уходит
            // фокусированному дочернему контролу, а не главному окну.
            if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN {
                let vk = msg.wParam.0 as i32;
                let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0;
                if ctrl && vk == 0x46 { // Ctrl+F — Найти
                    state.handle_command(CMD_FIND);
                    continue;
                }
                if vk == 0x7A { // F3 — Найти следующий
                    state.handle_command(CMD_FIND_NEXT);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
