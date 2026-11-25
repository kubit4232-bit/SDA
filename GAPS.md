# Отчёт по gap-анализу SDA vs IDM (по REA-данным)

Дата: 2026-11-20. Цель: `C:\Program Files (x86)\Internet Download Manager\IDMan.exe`
(PE, x86, sha256 `99a2410fd4fb9d88dbe624794bf7679731e0d1cf0acd1faa728de19b0065408e`).

## Что реализовано в текущем коде (в рамках сессии)

| Область | Файл / функция | Статус |
|---|---|---|
| Прогресс загрузки | `src/download/control.rs`, `engine.rs`, `manager.rs` | ✅ Реальный прогресс, скорость (окно 500 мс), таймауты |
| Пауза | `JobControl::set_paused`, `engine.wait_if_paused` | ✅ Реальная пауза потока (тестировано интеграционным тестом) |
| Возобновление | `manager.resume_job` (в т.ч. после «перезапуска» через реестр) | ✅ Тест пройден |
| Отмена | `JobControl::cancel`, abort handle | ✅ |
| Speed Limit | per-segment делим лимит на сегменты, sleep на ожидание | ✅ Тест скорости ~1.0 МБ/с ограничения |
| Максимум одновременных загрузок | `Gate` + `max_concurrent_downloads` (IDM: 2–16) | ✅ |
| Повторы при ошибке | `max_retries` + `_cancelled` протокол | ✅ |
| Сохранение списка задач | `state_file` (`%LOCALAPPDATA%\SDA\jobs.json`), `save_jobs`/`load_jobs` | ✅ Тест `job_list_survives_restart` |
| Последовательность/очередь | gate по лимиту → остальные в `Queued` | ✅ |
| Сортировка колонок | `header_cell`, `sort_col/asc`, `apply_sort` | ✅ (IDMan: OnColumnclickUrlList) |
| Категории | `category_for_path` (video/music/programs/documents/other) | ✅ |
| Поиск | `search` в GUI | ✅ |
| Батч-загрузка | `add_batch` (N ссылок) | ✅ |
| Монитор буфера обмена | `watch_clipboard` + таймер | ✅ |
| Настройки | GUI-панель + `DownloadConfig::load/save` + `settings.json` | ✅ |
| Скорость/сегменты/конкурентность в настройках | ✅ |
| Иконки SVG (resvg) | `src/gui/icons.rs`, `resources/icons/*.svg` | ✅ |

## Что в работе (доработано в этой сессии)

- Ядро `engine` переписано: реальное стриминговое скачивание, прогресс, пауза/отмена/лимит,
  `_CANCELLED` протокол (`CANCELLED` const).
- Менеджер: `JobControl` per job, `persist_jobs`, исторические задачи в реестре с автопаузой.
- `DownloadConfig` расширен (`max_concurrent_downloads`, `watch_clipboard`, `#[serde(default)]`).

## Оставшиеся разрывы vs IDM (по REA-архитектуре)

| Функция IDM | Статус в SDA | Что нужно сделать |
|---|---|---|
| Сетевой граббер (Grabber) — перехват URL из браузера | 🟡 Частично (SVG + меню) | Встроенный захват окон браузера / расширение |
| Планировщик запуска/отмены задач (Scheduler) | ✅ GUI-реализован (время старта/остановки, settings.json) | Внешний cron / интеграция с очередями |
| Прокси / SOCKS / Dial-Up VPN настройки | ✅ Частично (прокси HTTP в HttpClient, поле в конфиге) | UI-поля прокси/SOCKS в настройках |
| Пароли сайтов / Basic Auth | ✅ Реализовано (`HttpClient::with_basic_auth`, `DownloadJob.username/password`, `--user/--pass`, IPC) | Тест `basic_auth_is_enforced_and_accepted` ✅ |
| Пароли сайтов / cookie-флоу | 🟢 Частично (Basic Auth ✅ + Cookie-цепочка ✅ в движке, GUI, CLI, IPC; persist в jobs.json) | auth-хедеры, cookies |
| Уведомления (баллун/toast при завершении) | 🔴 Не реализовано | Windows toast |
| Трей (tray) + «Start with Windows» | 🔴 Не реализовано | eframe tray API / registry |
| Очередь «Q» (настройки очереди, reorder) | 🟡 Упрощённо (gate) | Drag&drop reorder |
| Копирование/добавление файла (Add from file) | 🔴 | file picking |
| Резервная копия/переименовать/открыть файл | 🟡 Частично (Open/Remove) | rename |
| История завершённых | 🟡 Через jobs.json | UI |

## Поведение, исправленное в этой сессии

- **GUI больше не падает** после «Скачать» (tokio runtime, clipboard).
- **Прогресс отображается** (раньше всегда 0), скорость реальная, ETA.
- **Пауза действительно останавливает** передачу (раньше менял статус мишени).
- **Лимит скорости** применяется при сегментации.
- **Файл сохраняется** с именем из URL / при заданном пути (раньше `sda_download_<uuid>` без расширения).

## Верификация

- `cargo build --release --bin sda --bin sda-gui --bin msg_host --features gui` — ✅
- `cargo test --test engine_pause` — ✅ 2 passed (pause/resume/complete, persistence).
- `sda.exe download … --limit-mbps 1` — ✅ (прогресс, лимит ~1.0 МБ/с, файл корректный).
- `sda.exe list` — ✅ Показывает задачу из реестра.
- `sda-gui.exe` — ✅ запущен и жив.
- Установщик — перепаковать после изменения кода.

## Следующие шаги (приоритет)

1. Сборка + установка `SDA-Setup-x64.exe`.
2. Вернуть GUI в установленную папку (`C:\Program Files\Super Download Agent (SDA)\`).
3. Планировщик (Scheduler) — время старта/стоп. ✅ GUI-реализован.
4. Прокси/SOCKS, уведомления, трей. ✅ Прокси, уведомления (in-app toast) — реализованы.
5. Сохранение настроек GUI в settings.json. ✅
6. Браузерный grabber/расширение.
7. Push кода в GitHub (github.com/kubit4232-bit/SDA).
