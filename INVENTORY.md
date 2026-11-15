# IDM Реверс-проект — Инвентарь инсталляции

Источник: `C:\Program Files (x86)\Internet Download Manager`
Дата проверки: 2026-11-08

## Основной исполняемый модуль (главный таргет)
| Файл | Размер | Дата | Роль (предварительно) |
|---|---|---|---|
| `IDMan.exe` | 4 034 616 | 06.03.2019 | Ядро: менеджер загрузок, UI, планировщик, COM-автоматизация (`idmantypeinfo.tlb` — type library) |

## Хелперы / службы процессов
| Файл | Роль |
|---|---|
| `IDMGrHlp.exe` | GRABBER-хелпер (site grabber / видео-хелпер?) |
| `idmBroker.exe` | Брокер между процессами/брайзерами и ядром |
| `IDMMsgHost.exe` | Native messaging host для браузерных расширений (`IDMMsgHost.json`, `IDMMsgHostMoz.json`) |
| `IEMonitor.exe` | Монитор IE |
| `IDMIntegrator64.exe` | Интегратор 64-битных браузеров |
| `MediumILStart.exe` | Launcher с понижением IL (medium integrity) — оркестрация прав |

## Браузерная интеграция (Chrome/Opera/Firefox/IE)
- `IDMGCExt.crx` (Chrome), `IDMOpExt.nex` (Opera), `idmmzcc3.xpi` / `idmmzcc2.xpi` (Firefox)
- `idmbrbtn.dll` / `idmbrbtn64.dll` — toolbar button
- `idmmzcc7.dll` — Firefox МО denomination
- `IDMIECC.dll` / `64` — IE Common Control/COAddon
- `IEExt.htm`, `IEGetAll.htm`, `IEGetVL.htm`, `IEGetVL2.htm` — entry points контекстных команд
- `downlWithIDM.dll` / `64` — shell/desktop hooks

## Сетевые драйверы и фильтры трафика (два стека!)
| Файл | Дата | Роль |
|---|---|---|
| `idmtdi32.sys` / `idmtdi64.sys` (`idmtdi.cat`, `.inf`) | 19.12.2018 | TDI-фильтр (устаревший стек) |
| `idmwfp32.sys` / `idmwfp64.sys` (`idmwfp.cat`, `.inf`) | 20.12.2018 | WFP callout/подграфики (классификация соединений браузеров) |
| `IDMNetMon.dll` / `64` | 10.02.2019 | Consumers/user-mode для фильтров |
| `idmfsa.dll`, `idmftype.dll`, `IDM64.dll`, `IDMFType.dat` | | File type / FSA модуль (категоризация типов файлов) |

## IE-расширения и shell
- `IDMShellExt.dll` / `64` — контекстное меню проводника
- `IDMGetAll.dll` / `64` — Get All (site grabber)
- `idmindex.dll` — индексация/база
- `idmmkb.dll` —多媒体 conversion helper
- `idmvs.dll` — Visual Studio integration helper
- `idmvconv.dll` — аудио/видео конвертер
- `idmcchandler2.dll` / `7.dll`, `64` — модули перехвата браузеров (handler версии 2 / 7)

## Точная версионная карта (из PE VersionInfo)
| Файл | Версия | Описание |
|---|---|---|
| `IDMan.exe` | 6.32.6.2 | Ядро (сборка Mar 2019) |
| `IDMNetMon.dll/.64` | 6.32.6.168 | Network Monitor (синхронизирован с ядром) |
| `IDMIECC.dll/.64` | 6.32.2.1 | IE Browser Helper Object |
| `idmBroker.exe`, `idmcchandler7.dll`, `downlWithIDM.dll` | 6.32.1.1 | Broker / click catcher v7 / IE menu |
| `idmtdi/sys + idmwfp sys` | 6.32.3.80 | Оба драйвера из одной сборки 6.32.3 |
| `IDMGrHlp.exe` | 6.31.8.1 | хелпер |
| `idmbrbtn.dll` | 6.31.10.62 | toolbar panel |
| `IDMMsgHost.exe` | 6.31.9.2 | Native Messaging Host |
| `IDMShellExt.dll` | 6.30.9.22 | shell extension |
| `idmvs.dll` | 6.30.6.1 | version module |
| `idmfsa.dll` | 6.20.3.1 | assistant |
| `idmvconv.dll` | 6.25.6.1 | конвертер |
| `idmindex.dll` | 6.23.21.1 | индексация |
| `idmcchandler2.dll` | 6.23.23.1 | click catcher v2 |
| `idmmzcc7.dll` | 6.23.18.1 | Firefox click catcher |
| `IDMGetAll.dll` | 6.18.9.1 | site grabber |
| `idmmkb.dll` | 6.19.9.1 | модуль |
| `idmftype.dll` | 6.15.6.5 | модуль типов |
| `Uninstall.exe` | 6.32.6.1 | установщик |

Наблюдение: ретро-сборка 6.32.6.2 — компоненты собраны в разные эпохи (6.15–6.32), т.е. ядро монолитно обновляется, а периферийные DLL переиспользуются почти без изменений.

## Параметры и локализация
- Все конфиги/кью (серийники, настройки) — в HKCU реестра, файлы не в папке install; `defexclist.txt` — список исключений

## Открытые вопросы для анализа
1. Из какой версии сборки IDM? (Верси — из PE resources, см. PE версии)
2. Кто поднимает драйверы TDI/WFP и как определить прямые браузеры?
3. Что делает `IDMFType.dat` (бинарный формат)?
4. COM-интерфейсы ядра через `idmantypeinfo.tlb`
