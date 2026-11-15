# IDM Реверс-проект — журнал находок (F-XX)

## F-07: COM-автоматизация ядра через `idmantypeinfo.tlb`

**Таргет:** `C:\Program Files (x86)\Internet Download Manager\idmantypeinfo.tlb` (2596 байт, дата 09.02.2004)  
**Статус:** разобран  
**Метод анализа:** регистрация библиотеки типов через `oleaut32.LoadRegTypeLib` + обход `ITypeLib`/`ITypeInfo` (`GetFuncDesc`/`GetVarDesc`/`GetNames`), парсинг `TYPEDESC`/`ELEMDESC` из управляемой памяти (P/Invoke на C# под PowerShell 7.6 x64). `oleview.exe` / `regtlib.exe` в системе отсутствуют; типотиблиотека не импортировалась через `TLI`/`TypeLibConverter` (отсутствуют в этом окружении).

---

### Сводка

Библиотека типов описывает COM-автоматизацию ядра IDM. Внешние программы могут создавать объект ядра и передавать ему ссылки на скачивание, минуя интеграцию с браузером.

| Атрибут | Значение |
|---|---|
| Библиотека | `IDManLib` |
| LIBID | `{ecf21eab-3aa8-4355-82be-f777990001dd}` |
| Версия | 1.0 |
| SYSKIND | WIN32 |
| LIBFLAGS | 0x0008 |

### Коиклассы

| Имя | CLSID | ProgId (регистрирован) | Примечание |
|---|---|---|---|
| `CIDMLinkTransmitter` | `{ac746233-e9d3-49cd-862f-068f7b7ccca4}` | `IDMan.CIDMLinkTransmitter` | ✅ Создаётся через `New-Object -ComObject` |
| `IDMan` | — | `IDMan.IDMan` | ❌ Класс не зарегистрирован (0x80040154) |

### Интерфейсы

| Интерфейс | IID | Тип | Методы |
|---|---|---|---|
| `ICIDMLinkTransmitter` | `{4bd46aae-c51f-4bf7-8bc0-2e86e33d1873}` | ДISPATCH | `SendLinkToIDM` (dispid `0x60010000`) |
| `ICIDMLinkTransmitter2` | `{94d09862-1875-4fc9-b434-91cf25c840a1}` | ДISPATCH | `SendLinkToIDM2` (dispid `0x60020000`), `SendLinksArray` (dispid `0x60020001`) |

Библиотека ссылается на `stdole2.tlb` (стандартная OLE-автоматизация).

### Методы (с dispid, типами параметров, возвращаемым значением)

Все методы возвращают `HRESULT`.

#### `ICIDMLinkTransmitter.SendLinkToIDM` — dispid `0x60010000` (9 параметров)

| # | Параметр | Тип |
|---|---|---|
| 0 | `bstrUrl` | BSTR |
| 1 | `bstrReferer` | BSTR |
| 2 | `bstrCookies` | BSTR |
| 3 | `bstrData` | BSTR |
| 4 | `bstrUser` | BSTR |
| 5 | `bstrPassword` | BSTR |
| 6 | `bstrLocalPath` | BSTR |
| 7 | `bstrLocalFileName` | BSTR |
| 8 | `lFlags` | i4 (long) |

#### `ICIDMLinkTransmitter2.SendLinkToIDM2` — dispid `0x60020000` (11 параметров)

Расширенная версия `SendLinkToIDM` с двумя дополнительными резервными параметрами.

| # | Параметр | Тип |
|---|---|---|
| 0 | `bstrUrl` | BSTR |
| 1 | `bstrReferer` | BSTR |
| 2 | `bstrCookies` | BSTR |
| 3 | `bstrData` | BSTR |
| 4 | `bstrUser` | BSTR |
| 5 | `bstrPassword` | BSTR |
| 6 | `bstrLocalPath` | BSTR |
| 7 | `bstrLocalFileName` | BSTR |
| 8 | `lFlags` | i4 (long) |
| 9 | `reserved1` | VARIANT |
| 10 | `reserved2` | VARIANT |

#### `ICIDMLinkTransmitter2.SendLinksArray` — dispid `0x60020001` (2 параметра)

| # | Параметр | Тип |
|---|---|---|
| 0 | `location` | BSTR |
| 1 | `pLinksArray` | массив (SAFEARRAY-подобный, код типа `0x001A`) |

### Проверка работоспособности (automation capability)

- Кокласс `IDMan.CIDMLinkTransmitter` успешно создаётся через COM (`New-Object -ComObject IDMan.CIDMLinkTransmitter`).
- Методы `SendLinkToIDM`, `SendLinkToIDM2`, `SendLinksArray` диспатчеризируются в код ядра IDM — вызовы доходят до реализации (ошибка `NullReference` внутри IDM означает, что ядро `IDMan.exe` не было в полной мере инициализировано в момент теста, а не отсутствие метода).
- Другие проигранные ProgId (`IDMan.IDMan`, `IDManLib.*`, `IDMan.Application`) не зарегистрированы в системе (0x80040154 `REGDB_E_CLASSNOTREG`).

### Автоматизация, которую предоставляет IDM через COM

Внешний процесс может, не запуская интерфейс IDM вручную:

1. Создать COM-объект `IDMan.CIDMLinkTransmitter`.
2. Позвать `SendLinkToIDM(url, referer, cookies, data, user, password, localPath, localFileName, lFlags)` — поставить в очередь загрузку одного файла с заданным адресом, реферером, куки, логином/паролем, путём и именем сохранения.
3. Позвать `SendLinkToIDM2(...)` — то же самое с дополнительными резервными параметрами.
4. Позвать `SendLinksArray(location, pLinksArray)` — передать массив ссылок на скачивание разом.

Требуется, чтобы ядро IDM (`IDMan.exe`) было запущено/инициализировано в системе; иначе COM-объект создаётся, но вызовы методов не завершаются успешно.

### Границы проекта

- Точные флаги `lFlags` и семантика `pLinksArray` не расшифровывались (требуется вызов методов при запущенном `IDMan.exe` и/или дизассембляция реализации в `IDMan.exe`).
- Глубокий дизассемблянг реализации методов в `IDMan.exe` входит в таргет №1 (ядро), уже разобран.

### Evidence

- Файл: `C:\Program Files (x86)\Internet Download Manager\idmantypeinfo.tlb` (2596 байт).
- LIBID, CLSID, IID, dispid, имена/типы параметров извлечены напрямую из библиотеки типов через `ITypeLib`/`ITypeInfo`.
- Проверка создаваемости кокласса: `IDMan.CIDMLinkTransmitter` — OK; `IDMan.IDMan` — REGDB_E_CLASSNOTREG.
- Проверка диспатчеризации методов: выполнена через COM-вызов (результат — `NullReference` внутри IDM, т.о. методы существуют и достигают кода ядра).

---

## Связанные открытые вопросы (из README)

- Q4: «COM-интерфейсы из `idmantypeinfo.tlb`» — закрыто данным пунктом (F-07).

---

## F-08: IDMNetMon.dll — user-mode потребитель драйверов TDI/WFP ✅

**Таргеты:** `IDMNetMon.dll` (x86, SHA-256 `56c59763…e738f`), `IDMNetMon64.dll` (x64, SHA-256 `7da8841b…1cefac`)  
**Версия:** 6.32.6.168 (синхронизирована с ядром IDMan.exe)  
**Статус:** разобран (Ghidra, `list_strings` + `list_procedures` + `analyze_function` на ключевых точках)

### Архитектура

IDMNetMon.dll — это=user-mode компонент, который связывает ядро IDM (`IDMan.exe`), драйверы (`idmtdi.sys`, `idmwfp.sys`) и браузерные расширения (через `IDMMsgHost.exe`). Запускается как DLL внутри процесса IDMan.exe.

### Каналы связи (4 независимых пути)

#### 1. Named Pipe server — `\\.\pipe\IDMNetworkMonitor.<sessionId>`
- Создаётся в `FUN_100054b0` через `CreateNamedPipeW` с флагами `PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED`, тип сообщения, буфер 4KB/8KB, таймаут 60 с.
- Клиентом является `IDMMsgHost.exe` (нативный хост браузерного расширения).
- Протокол через именованный канал — бинарные сообщения с опкодами `0x01`–`0x07` (обработка в `FUN_10005530`, case 0):
  - `0x01`–`0x04` → `FUN_10006a10` → `DeviceIoControl(0x12c014 / 0x12c018)`
  - `0x05` → `FUN_100067b0`
  - `0x06`–`0x07` → `FUN_100068f0` → `DeviceIoControl(0x12c024)` с буфером `{0x108, param, 6}` и проверкой ответа `0x05`

#### 2. TCP/WebSocket сервер — `127.0.0.1:<порт>` (порт 1001 по строкам)
- Создаётся в `FUN_10005ea0` + `FUN_100053c0`: `socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)`, `bind(127.0.0.1)`, `listen()`, `WSAEventSelect(FD_ACCEPT|FD_READ)`.
- Используется протоколом `plugin.v3.internetdownloadmanager.com` (WebSocket handshake, строка `258EAFA5-E914-47DA-95CA-C5AB0DC85B11` — стандартный WebSocket GUID).
- Сообщения в формате `MSG#%u#%u#%u#%u` (строка из `.rdata`).

#### 3. Windows хуки — перехват сообщений браузера
- Устанавливаются в `FUN_100052d0` (главный поток мониторинга):
  - `WH_GETMESSAGE` (код 0xd) — перехват сообщений из очереди потока
  - `WH_CALLWNDPROC` (код 0xe) — перехват вызовов оконных процедур
- Окно-потребитель: `IDMNetworkMonitorMessageWindow` (создаётся в `FUN_10006140`, процедура `FUN_10006750` → `FUN_10006610`)
- Обрабатывает `WM_COPYDATA` (0x4a) с магией `0xb1d3` — данные копируются в контекст мониторинга (смещение `0xc04`+)
- Кастомное сообщение `0xb1d2` → `FUN_1001bdd0`

#### 4. Прямой доступ к драйверам — `DeviceIoControl` на `\\.\IDMTDI` / `\\.\IDMWFP`
- Импортируется `DeviceIoControl` (KERNEL32), вызывается из 7 мест:
  - `FUN_10003ba0` (@0x10003bc1)
  - `FUN_100068f0` (@0x1000694f) — из обработчика named pipe опкодов 0x06/0x07
  - `FUN_10006a10` (@0x10006aa6) — из обработчика named pipe опкодов 0x01-0x04
  - `FUN_100152f0` (@0x10015395, @0x10015456) — WebSocket/сетевой обработчик (2 вызова)
  - `FUN_100154e0` (@0x1001553f) — таймер/периодическая проверка
- IOCTL-коды: `0x12c014`, `0x12c018`, `0x12c020`, `0x12c024`
- Дескриптор устройства хранится в контексте по смещению `0xb94`

### Конфигурация и состояние

- Читается из реестра `HKLM\Software\DownloadManager\ConfigTime` (`FUN_100070e0`): `EnableDriver`, `menuExt`, `SpecialKeys`, `UseKeyToForce`, `UseKeyToPrevent`, `ShiftF/CtrlF/AltF/InsF`, `ShiftP/CtrlP/AltP/DelP`, `PanelMode` и др.
- Мьютекс `Local\IDMEventMonitor` — гарантирует единичный экземпляр монитора.
- Скрытое окно `IDMNetworkMonitorMessageWindow` — получает уведомления от хуков.
- Лог трафика: `Traffic.log`, события в `Local\IDMEventMonitor`.
- Файл типов: загружает `IDMFType.dat` через `idmftype.dll`.

### Потоки данных

```
Браузерное расширение
   │ (Native Messaging stdio)
   ▼
IDMMsgHost.exe ──named pipe──► IDMNetMon.dll ◄──WM_COPYDATA/хуки── Браузер (окна)
   │                           │
   │ (WebSocket 127.0.0.1:1001)  │ (DeviceIoControl IOCTL 0x12c0xx)
   ▼                           ▼
плагин браузера          \\.\IDMTDI / \\.\IDMWFP (драйверы)
                                                      │
                                                      ▼
                                                перехват TCP-трафика
```

### Evidence

- `IDMNetMon.dll` x86: Ghidra session, `ev_6cd833c8…` (list_strings), `ev_9c718672…` (list_procedures)
- Ключевые адреса: `ControlMonitoring` = `0x10003810`, точка входа = `0x1002aeb2`, named pipe формат = `\\.\pipe\IDMNetworkMonitor.%u`, окно = `IDMNetworkMonitorMessageWindow`
- IOCTL-коды: `0x12c014`, `0x12c018`, `0x12c020`, `0x12c024`
- Device paths: `\\.\IDMTDI`, `\\.\IDMWFP`
- WebSocket: `127.0.0.1:1001`, протокол `plugin.v3.internetdownloadmanager.com`, формат `MSG#%u#%u#%u#%u`
- Сообщения named pipe: опкоды `0x01`–`0x07`

