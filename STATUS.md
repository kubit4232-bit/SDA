# Обновлённый таргет-план SDA (вариант B — Rust, Windows x64)

| # | Компонент | Статус | Путь к коду |
|---|-----------|--------|-------------|
| 1 | SDA Core (Rust, tokio) — ядро загрузчика | ✅ MVP собран | `SDA/src/` |
| 2 | Сегментированная загрузка (Range) | ✅ | `SDA/src/download/engine.rs` |
| 3 | Resume (возобновление) | ✅ Логика реализована | `SDA/src/download/state.rs` |
| 4 | CLI-интерфейс (download/list/status/serve) | ✅ | `SDA/src/main.rs` |
| 5 | IPC Named Pipe `\\.\pipe\SDANetworkMonitor` | ✅ Полный цикл реализован | `SDA/src/main.rs` |
| 6 | MsgHost (Native Host) | ✅ Сборка проходит | `SDA/src/msg_host/main.rs` |
| 7 | Браузерное расширение Chrome MV3 | ✅ Упаковано в .zip/.crx | `SDA/extension/`, `SDA/build/extension/` |
| 8 | Установщик Windows x64 (Inno Setup) | ✅ Собран | `SDA/build/installer.iss` → `SDA/build/output/SDA-Setup-x64.exe` |
| 9 | GUI (egui, IDM-подобный) | ✅ Собран | `SDA/src/gui/`, `SDA/src/gui_main.rs` |
| 9a | GUI native Win32 (1:1 контролы IDM) | ✅ Собран, запущен | `SDA/src/native_gui.rs`, `SDA/src/gui_main.rs` |
| 10 | Кастомная иконка (app.ico) | ✅ Встроена в PE ресурсы | `SDA/resources/app.ico`, `SDA/resources/app.rc`, `SDA/build.rs` |
| 11 | Вставка из буфера обмена | ✅ | `SDA/src/gui/app.rs`, `arboard` crate |
| 12 | YouTube/yt-dlp поддержка | ✅ | `SDA/src/download/youtube_extractor.rs`, `yt-dlp.exe` bundled |

## Финальное состояние (все шаги завершены ✅)

- **Бинарник CLI** (`sda.exe`): 2,9 МБ, Windows x64, release, с кастомной иконкой
- **Бинарник MsgHost** (`msg_host.exe`): 445 KB, Windows x64, release, с кастомной иконкой
- **Бинарник GUI** (`sda-gui.exe`): 4,6 МБ, Windows x64, release, с кастомной иконкой
- **Иконка**: `SDA/resources/app.ico` (256×256, синий круг с буквой "S")
- **Установщик**: `build/output/SDA-Setup-x64.exe` (4,6 МБ, Inno Setup 7.1.1, ярлыки с иконкой)
- **Расширение**: `E:\site\IDM\SDA\build/extension/sda-extension.zip` + `.crx`
- **yt-dlp.exe**: 23,6 МБ (встроен для YouTube/видеохостингов)

## GUI — нативный Win32 (Rust + windows-rs)

- Переписан с egui на **нативные контролы Win32** — те же, что использует IDM:
  `SysTreeView32` (категории), `SysListView32` (список с колонками), `ToolbarWindow32`,
  `msctls_statusbar32`, меню через `CreateMenu/AppendMenuW`.
- Меню: `Задачи / Скачивание / Вид / Помощь / Регистрация`.
- Колонки: `Имя файла | Очередь | Размер | Состояние | Осталось... | Скорость | Дата последней попытки | Описание`.
- Категории: 10 пунктов, включая «Посещены Граббером» и «Очереди».
- Диалог добавления: URL / путь / сегменты / логин / пароль / cookie.
- Диалог настроек с вкладками: `Общие / Соединение / Прокси-Соксы / Пароли сайтов / Резерв-Сканирование / Автозвонок / Интеграции / Скачивание`.
- Таймер обновления списка 500 мс, двойной клик — открыть файл.

## GUI — функции (IDM-подобный интерфейс)

| Функция | Описание |
|---------|----------|
| Панель инструментов | Кнопки Скачать, Пауза, Возобновить, Отмена — как в IDM |
| Меню приложения | Задачи, Скачивание, Сервис, Справка |
| Поле ввода URL | В главном окне с кнопкой "Скачать" |
| Вставка из буфера обмена | Кнопка "Вставить" — вставляет ссылку из буфера обмена |
| Левая панель категорий | Все загрузки, Скачивающие, Документы, Музыка, Программы, Видео, Незавершённые, Завершённые, Ожидающие, Ошибки, Очередь, Другие |
| Список загрузок | Статус, Имя файла, Источник, Прогресс, Размер, Скорость, Осталось, Действия |
| Сортировка колонок | Клик по заголовку (Статус, Имя, Источник, Прогресс, Размер, Скорость, Осталось) |
| Поиск по списку | Под полем ввода |
| Планировщик | GUI-панель Scheduler (время старта/остановки, в settings.json) |
| Уведомления | Toast при завершении задачи («Задача завершена: <файл>») |
| Батч-загрузка | Мультиссылка в одной операции |
| Кнопки в строках | Пауза/Возобновить, Отмена/Удалить для каждой загрузки |
| Автообновление | Список обновляется каждые 100 мс |
| Тёмная/светлая тема | Светлая (как в IDM) |
| Иконки | SVG из `resources/icons/*.svg`, рендер через resvg в textures egui (цвета IDM: `#5b7fa8`, `#c0392b`) |

## Исправления (текущий статус)

- **Вылет GUI после нажатия «Скачать»** устранён: `futures::executor::block_on` заменён на
  `tokio::runtime::Runtime::block_on` (runtime добавлен в `SdaApp`).
- **Иконки**: эмодзи заменены на SVG-иконки (tasks, download, start, stop, stop_all, refresh, filename,
  settings, schedule, launch, grabber, categories, clipboard, pause, music, programs, video), загружаемые
  через resvg → egui texture.
- **Прогресс**: реальный прогресс/скорость/ETA вместо константных «0».
- **Пауза**: реальная пауза потока передачи, возобновление, отмена.
- **Speed Limiter**: лимит скорости (MB/s) применяется при сегментированной загрузке.
- **Конкурентность**: лимит одновременных загрузок (до 16).
- **Сохранение списка**: `%LOCALAPPDATA%\SDA\jobs.json` + восстановление при старте.
- **Настройки**: GUI-панель сохраняет/загружает `%LOCALAPPDATA%\SDA\settings.json`.
- **Батч/поиск/категории**: пакетная загрузка, поиск, категории по расширению.
- **Cookie-аутентификация**: ✅ Реализована (engine → `HttpClient::with_cookie_header`, `DownloadJob.cookies`, GUI-поля «Cookie», CLI `--cookie`, IPC `DownloadRequest.cookies`, persist в `jobs.json`, resume сохраняет auth).

## REA-анализ IDM (интерфейс)

- Цель: `C:\Program Files (x86)\Internet Download Manager\IDMan.exe` (PE, x86, sha256 `99a2410f...`).
- Статический анализ через Ghidra (REA): `CDownloaderDlg`, `CDownload`, `CDownloadComponents`,
  `CDownloadQueue`, `CDownlProgressDlg`, `CDownloadProgressComponent`, `CListCtrl` (список загрузок),
  `OnColumnclickUrlList`, колонка `Speed`, строка `"Time left"`, колонка `"Q"` (очереди).
- Runtime-наблюдение UI через REA недоступно на Windows (только macOS) — использован статический анализ строк.

## REA-воркфлоу (интеграция в разработку)

- Процесс реверс-инжениринга зафиксирован в `REA-WORKFLOW.md` (сессия REA → находки → GAPS → реализация → тесты).
- Каталог методов и кейсов: Awesome-REA (`https://github.com/reacher-z/Awesome-REA`, https://awesomerea.org) — источник готовых методов перед анализом новых областей.
- REA — часть процесса разработки SDA: анализ IDM → задачи → фичи в `src/` → тесты → статус.

## Тестирование

| Тест | Результат |
|------|-----------|
| Скачивание обычного файла (с Content-Length) | ✅ Работает |
| Скачивание YouTube Shorts | ✅ Работает (через yt-dlp) |
| Сегментированное скачивание (8 потоков) | ✅ Работает |
| Resume (возобновление загрузки) | ✅ Тест `tests/engine_pause.rs` |
| Пауза реально останавливает поток | ✅ Тест `pause_stops_transfer_and_resume_completes` |
| Сохранение списка задач | ✅ Тест `job_list_survives_restart` |
| Speed limit (MB/s) | ✅ CLI `--limit-mbps`, ~1.0 МБ/с в тесте |
| GUI запуск и интерфейс | ✅ Работает |
| CLI интерфейс | ✅ Работает |

## Сборка

```powershell
$env:PATH = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64;" + $env:USERPROFILE + "\.cargo\bin;" + $env:PATH
$env:LIB = "C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64;" + $env:LIB
cd E:\site\IDM\SDA
cargo build --release
cargo build --bin sda-gui --features gui --release
```

Для сборки установщика:
```powershell
& "C:\Users\Skynet\AppData\Local\Programs\Inno Setup 7\ISCC.exe" "E:\site\IDM\SDA\build\installer.iss"
```

## Границы проекта (по REA выводам)

- Не используется исходный код IDM — только архитектурные принципы, выявленные через REA (сегменты, named pipe, native messaging)
- Обход лицензионной защиты IDM не выполнялся — это отдельный коммерческий продукт Tonec Inc.
- Цифровая подпись бинарников не выполнена (требует EV-сертификата)
