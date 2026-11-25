# SDA (Super Download Agent)

Проект переосмысления архитектуры Internet Download Manager (IDM) на современном высокопроизводительном движке (Rust, Windows x64).

## Архитектура (на основе результатов REA-анализа IDM)

```
Браузер (Chrome/Edge/Firefox)
      │ Native Messaging (stdio, JSON с 4-байтной длиной)
      ▼
SDA MsgHost (IDMMsgHost.exe аналог)
      │ Named Pipe \\.\pipe\SDANetworkMonitor
      ▼
SDA Core (ядро загрузчика, этот репозиторий)
      │ HTTP/1.1, HTTP/2, Range-запросы
      ▼
Удалённый сервер (CDN, хостинг)
```

Ключевые решения, перенёсённые из IDM и переосмысленные:
- **Сегментированная загрузка**: файл режется на N сегментов, каждый качается параллельным потоком с `Range:` заголовком (аналог `IoBuildDeviceIoControlRequest` в IDM TDI-драйвере).
- **Resume**: состояние задачи сохраняется на диск, при обрыве загрузка продолжается с места остановки.
- **IPC**: именованные каналы Windows (`\\.\pipe\SDANetworkMonitor.%u`) для связи с браузерным расширением/MsgHost.
- **Без драйвера в MVP**: используется пользовательский режим (WinDivert опционально), чтобы не нуждаться в подписи драйвера.

## Лицензия и данные

- **SDA** — самостоятельный реинтерпретационный проект. Архитектурные принципы Internet Download Manager получены через анализ (REA/Ghidra) и чистая реализация на Rust.
- **Не используется**: исходный код IDM, лицензионная информация/ключи IDM, лицензионные материалы.
- **Не используется**: слитые (пиратские) данные, кейсы и ресурсы сторонних источников для функциональности клона.
- Клон делается «1 в 1» по функциональности, но без лицензии и без слитых данных.

## Статус

| Компонент | Статус |
|---|---|
| Ядро загрузчика (Rust, tokio) | ✅ MVP реализован |
| Сегментирование (Range) | ✅ |
| Resume (возобновление) | 🔄 Частично (логика есть, состояние на диске в разработке) |
| Браузерное расширение | ⏳ Планируется |
| MsgHost (Native Messaging) | ⏳ Планируется |
| Named Pipe IPC | 🔄 Скелет в `src/main.rs` |
| Установщик Windows x64 | 🔴 `build/installer.iss` подготовлен |

## Сборка

```powershell
# Установка Rust (если не установлен)
# см. https://rustup.rs/

cd E:\site\IDM\SDA
cargo build --release
```

Бинарник будет в `E:\site\IDM\SDA\target\release\sda.exe`.

## Использование (CLI)

```powershell
# Скачать файл
.\target\release\sda.exe download https://example.com/file.zip --output ./file.zip --segments 8

# Список задач
.\target\release\sda.exe list

# Статус задачи
.\target\release\sda.exe status <job-id>

# Режим IPC (для браузерного расширения)
.\target\release\sda.exe serve --pipe-name \\.\pipe\SDANetworkMonitor
```

## Браузерное расширение (Chrome / Edge / Firefox)

Исходники расширения находятся в `E:\site\IDM\SDA\extension/`. Готовый пакет для установки: `E:\site\IDM\SDA\build\extension\sda-extension.zip` (также дублирован как `.crx` для разработки).

Архив собирается из `manifest.json`, `background.js`, `content.js` и `com.sda.agent.json` (`README.md` из папки расширения не включается).

### Google Chrome / Microsoft Edge (Manifest V3)

1. Откройте `chrome://extensions` (Chrome) или `edge://extensions` (Edge).
2. Включите **Режим разработчика** (переключатель в правом верхнем углу).
3. Нажмите **Распакованное расширение** → выберите папку `E:\site\IDM\SDA\extension/`, либо **Загрузить распакованное** и укажите эту папку.
4. Расширение "SDA Integration Module" появится на панели расширений.

> Для установки из `.zip`/.crx без интернет-магазина в Chrome/Edge может потребоваться флаг `--disable-extensions-except` или загрузка в режиме разработчика, так как браузеры блокируют установку CRX из сторонних источников.

### Mozilla Firefox

1. Откройте `about:debugging#/runtime/this-firefox`.
2. Нажмите **Загрузить временное дополнение…** и выберите `manifest.json` из папки `E:\site\IDM\SDA\extension/`.
3. Дополнение будет активно до перезапуска браузера.

### Связь с ядром SDA

Расширение использует Native Messaging (`com.sda.agent.json`) и именованный канал `\\.\pipe\SDANetworkMonitor` для передачи задач загрузки в SDA Core. Запустите ядро перед использованием:

```powershell
.\target\release\sda.exe serve --pipe-name \\.\pipe\SDANetworkMonitor
```

## Сборка установщика

1. Установить [Inno Setup](https://jrsoftware.org/isinfo.php)
2. Собрать проект: `cargo build --release`
3. Запустить компиляцию: `ISCC E:\site\IDM\SDA\build\installer.iss`
4. Готовый установщик: `E:\site\IDM\SDA\build\output\SDA-Setup-x64.exe`

## Портативная версия

1. Собрать проект: `cargo build --release --bin sda --bin sda-gui --bin msg_host --features gui`
2. Запустить: `powershell -File E:\site\IDM\SDA\build\make_portable.ps1`
3. Готовый архив: `E:\site\IDM\SDA\build\output\SDA-Portable-x64.zip`, распакованная папка — `build\output\SDA-Portable-x64\`.

В папке есть файл-маркер `portable.txt`: пока он на месте, настройки (`settings.json`) и список загрузок (`jobs.json`) хранятся рядом с exe, а не в `%LOCALAPPDATA%\SDA`. После удаления маркера программа возвращается к обычному режиму.

## Лицензия

MIT
