# IDM Реверс-проект

Реверс-инжиниринг инсталляции **Internet Download Manager** (Tonec Inc.) на
`C:\Program Files (x86)\Internet Download Manager` — сборка ядра **6.32.6.2** (PE32 x86).

## Инструменты
- REA MCP (`tools.rea.*`) для анализа таргетов, преимущественно Ghidra 12.1.4 headless.
- REA CLI (`rea`, v6.7.0) для тяжёлых прогонов: таймаут стартового авт-анализа поднимается переменной `REA_GHIDRA_STARTUP_TIMEOUT_MS`.

## Файлы проекта
| Файл | Назначение |
|---|---|
| `INVENTORY.md` | полный инвентарь инсталляции + версионная карта всех PE |
| `FINDINGS.md` | журнал находок F-XX с Evidence-реестром и границами проекта |
| `idman.snapshot` | снапшот Ghidra-анализа ядра `IDMan.exe` |
| `analyze-idman.json` | полный Evidence-вывод `rea analyze` ядра |
| `ext/chrome/unpacked/` | распакованный `IDMGCExt.crx` (Chrome, MV2) |
| `searches/*.json` | сырые результаты поисковых запросов Ghidra (строки, процедуры) |

## Таргет-план (порядок)
| # | Таргет | Статус | Ключевые находки |
|---|---|---|---|
| 1 | `IDMan.exe` (ядро) | ✅ Готов | Ghidra-анализ завершён (17 763 proc, 5 661 str), снапшот `idman.snapshot` |
| 2 | `idmtdi32/64.sys` (TDI) | ✅ Разобран | `\\Device\\IDMTDI`, `\\DosDevices\\IDMTDI`; DriverUnload=FUN_00010e00; TDI прикрепление к `\\Device\\Tcp` (FUN_00010490); исходящие `IoBuildDeviceIoControlRequest(3,…)` из FUN_000203d0/00020600/00020b10/00020d70; реестр `RedirectAddr/RedirectPort/AddressHash/ConnectionHash` |
| 3 | `idmwfp32/64.sys` (WFP) | ✅ Разобран | `IoValidateDeviceIoControlAccess` + `ZwCreateFile`; без TDI-запросов — WFP callout стек |
| 4 | `IDMNetMon.dll/.64` | ✅ Разобран | NamedPipe `\\.\pipe\IDMNetworkMonitor.%u`, WebSocket 127.0.0.1:1001, хуки WH_GETMESSAGE/WH_CALLWNDPROC, DeviceIoControl IOCTL 0x12c0xx к TDI/WFP драйверам (см. FINDINGS.md F-08) |
| 5 | `IDMMsgHost.exe` + `.json` | ✅ Готов | Нативное сообщение (`com.tonec.idm`): браузер ↔ хост через stdio (4-байт длина + JSON); хост ↔ ядро через именованный канал `\\.\pipe\IDMNetworkMonitor` (CreateFileW/ReadFile/WriteFile, overlapped, потоки ввода/вывода); см. FINDINGS.md F-07 |
| 6 | `idmantypeinfo.tlb` | ✅ Готов | COM-автоматизация ядра: библиотека `IDManLib` (LIBID `{ecf21eab-...}`), кокласс `CIDMLinkTransmitter` (ProgId `IDMan.CIDMLinkTransmitter`), интерфейсы `ICIDMLinkTransmitter` / `ICIDMLinkTransmitter2` с методами `SendLinkToIDM` / `SendLinkToIDM2` / `SendLinksArray` (см. `FINDINGS.md` §F-07) |
| 7 | `IDMFType.dat` + DLL | ✅ Готов | TLV-база типов файлов: заголовок `IDMFTYPE`/вер.1, 7 записей (557 MIME-типов, 467 расширений, 2008 описаний, 842 метаданных, 2594-байтовый блоб, 3224×38-байтовая таблица), полезные нагрузчи потоковый XOR (ключ `0xE395A6C7`); парсер в `idmftype.dll` `FUN_10001000` (FINDINGS F-08) |
| 8 | Firefox `.xpi` / Opera `.nex` | ✅ Готов | Веб-билды (idmmzcc3, IDMOpExt) — код идентичен Chrome, манифесты близки; устаревшие XPI (idmmzcc, idmmzcc2) — XUL/XPCOM без веб-API и нативного хоста |

## Открытые вопросы
1. Точные IOCTL-коды TDI в `idmtdi.sys` (требуется чтение байт инструкций по адресам вызовов `IoBuildDeviceIoControlRequest`)
2. WFP callout-классы и слои в `idmwfp.sys` (требуется дизассембляция регистрации callout)
3. Протокол нативного сообщения `IDMMsgHost.exe` (структура сообщений между расширением и ядром)
4. COM-интерфейсы из `idmantypeinfo.tlb`

## SDA — ребрендинг и rewrite (вариант B, Rust)

Проект перенесён в `E:\site\IDM\SDA/`. Статус: см. `SDA/STATUS.md`.

Архитектура SDA повторяет принципы IDM, выявленные через REA:
- Сегментированная загрузка (Range-заголовки, N потоков)
- Named Pipe IPC (`\\.\pipe\SDANetworkMonitor`)
- Native Messaging мост для браузерного расширения
- Resume (возобновление загрузки)

## Примечания по работе с REA
- Ghidra 12.1.4 headless: стартовый анализ 4 МБ x86 требует `REA_GHIDRA_STARTUP_TIMEOUT_MS` > 330000 ms (использовали 1 800 000 ms через CLI).
- Windows Ghidra P0 не поддерживает мутации базы (annotate_* недоступны), но чтение/анализ доступны полностью.
- `search_strings` / `search_procedures` через CLI возвращают сырые JSON; в PowerShell лучше парсить `raw_result` через regex по `"value":"([^"]+)"`.
