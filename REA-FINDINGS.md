# REA-находки по IDMan.exe — визуальный 1:1 SDA

Дата: 2026-11-25. Цель: `C:\Program Files (x86)\Internet Download Manager\IDMan.exe`
(PE x86, sha256 `99a2410fd4fb9d88dbe624794bf7679731e0d1cf0acd1faa728de19b0065408e`).

## ledger Evidence

| Наблюдение | Тип | Evidence ID | Уверенность |
|---|---|---|---|
| PE-ресурсы: 387 записей, иконки 16/32/48 px, 8bpp и 32bpp; тулбар использует 32×32 | observed (shipped artifact) | `ev_f848ac906bc9df4550d85ccc39f643dea05602821738613425a8d0f66d1ce35d` | высокая |
| UI-строка `"Add URL"` (0x6caed0) | observed | `ev_5fb6d280bda062402a5c35a0e231ca445ca72cd9410ab764e33b8d87ac5ca426` | высокая |
| UI-строки `"Options"` (0x6cae9c), `"Pause"` (0x6cace0) | observed | `ev_d6...` (сессия 2026-11-25) | высокая |
| Вкладки Options: `Save to` (0x6ca6c0), `File types` (0x6c6518), `Scheduler` (Software\DownloadManager\Scheduler) | observed | `ev_d33224a20599e80e0748402e0017f60350813c99b02678c027c3a5af3af63d53`, `ev_348bbeb7876e0877d7bfcc11dc6c94c24e783d0da998b0`, `ev_47118078eda44757bb445eda2a3d7f0fd38d9c850f64264ab3a86665faf73186` | высокая |
| `CDownloaderDlg::OnOptions()`, `CListCtrl`, `OnColumnclickUrlList` | observed (static) | предыдущие сессии, STATUS.md | высокая |

## Применено к SDA

1. **Иконки**: было 16×16 с premultiplied alpha без маски → тёмные квадраты.
   Стало: 32×32 (по REA), композитинг на белый + `ImageList_AddMasked` с белой маской.
   Файл: `src/native_gui.rs` (`load_icon_bitmap`, `create_toolbar`).
2. **Чёрное окно при запуске**: GUI-бинарник собирался как console-subsystem.
   Добавлено `#![cfg_attr(windows, windows_subsystem = "windows")]` в `src/gui_main.rs`.
   Проверено: у процесса `sda-gui.exe` нет дочерних `conhost.exe`.
3. **Нативный GUI**: `SysTreeView32`, `SysListView32` с колонками из REA,
   `ToolbarWindow32`, `msctls_statusbar32`, меню, диалог добавления и 8 вкладок настроек.

## Ограничения

- REA на Windows даёт только статический анализ (runtime UI — только macOS).
- Скриншоты пользователя не читаются моделью — визуальное сравнение выполняет пользователь.
- Скопированные иконки IDM **не используются**: SVG-иконки SDA авторские (clean-room).
