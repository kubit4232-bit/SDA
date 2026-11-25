# SDA Native Messaging Host (MsgHost)

Мост между браузерным расширением (Chrome/Edge/Firefox) и ядром SDA (Rust).

## Архитектура

```
Браузерное расширение
      │ Native Messaging (stdio, JSON с 4-байтной длиной prefixed)
      ▼
SDA MsgHost (этот хост)
      │ Named Pipe \\.\pipe\SDANetworkMonitor
      ▼
SDA Core (Rust ядро загрузчика)
```

## Установка

### Chrome/Edge
Реестр: `HKEY_CURRENT_USER\Software\Google\Chrome\NativeMessagingHosts\com.sda.agent`
или `HKEY_LOCAL_MACHINE\Software\Google\Chrome\NativeMessagingHosts\com.sda.agent`

Файл: `com.sda.agent.json`:
```json
{
  "name": "com.sda.agent",
  "description": "SDA Native Messaging Host",
  "path": "C:\\Program Files\\SDA\\msg_host.exe",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://<ВАШ_ID_РАСШИРЕНИЯ>/"
  ]
}
```

### Firefox
Реестр: `HKEY_CURRENT_USER\Software\Mozilla\NativeMessagingHosts\com.sda.agent`

Файл: `com.sda.agent.json`:
```json
{
  "name": "com.sda.agent",
  "description": "SDA Native Messaging Host",
  "path": "C:\\Program Files\\SDA\\msg_host.exe",
  "type": "stdio",
  "allowed_extensions": [
    "sda-agent@example.com"
  ]
}
```

## Протокол

Все сообщения — JSON с 4-байтной длиной (little-endian) в начале, как в оригинальном IDM.

### Сообщения от расширения к ядру (через MsgHost)

| Тип | Поля | Описание |
|---|---|---|
| `download_request` | `url`, `referer?`, `cookies?`, `save_path?`, `tab_id?` | Запрос на скачивание файла |

### Сообщения от ядра к расширению (через MsgHost)

| Тип | Поля | Описание |
|---|---|---|
| `download_accepted` | `job_id` | Задача принята |
| `progress` | `job_id`, `downloaded`, `total?`, `speed_bps` | Обновление прогресса |
| `completed` | `job_id`, `path` | Загрузка завершена |
| `error` | `job_id?`, `message` | Ошибка загрузки |

## Реализация MsgHost

MsgHost реализуется на Rust как отдельный бинарник в репозитории SDA:

1. Читает сообщения из stdin (4-байтная длина + JSON)
2. Превращает их в `SdaMessage` и отправляет через Named Pipe в SDA Core
3. Читает ответы из Named Pipe и пишет в stdout (4-байтная длина + JSON)

Пример фрейма:
```
[0x2C 0x00 0x00 0x00] {"type":"download_request","url":"https://..."}
   └── длина = 44 байта
```

## Сборка

Добавьте в `Cargo.toml`:
```toml
[[bin]]
name = "msg_host"
path = "src/msg_host/main.rs"
```
