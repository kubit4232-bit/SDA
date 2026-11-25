# REA-воркфлоу SDA — интеграция reverse engineering в процесс разработки

Дата: 2026-11-25. Статус: **интегрирован в процесс разработки SDA**.

## 1. Суть подхода

```
Awesome-REA (каталог ресурсов, awesomerea.org / github.com/reacher-z/Awesome-REA)
        │  кейсы, методы, инструменты, готовые реконструкции
        ▼
REA (агент: Ghidra provider, Hopper)  ←  статический анализ целевого бинарника
        │  строки, процедуры, xrefs, инструкции, комментарии
        ▼
Находки → GAPS.md / STATUS.md  →  реализация в src/  →  тесты  →  сборка/установка
```

## 2. Цель анализа (текущая)

- **IDMan.exe**: `C:\Program Files (x86)\Internet Download Manager\IDMan.exe`
  (PE, x86, sha256 `99a2410fd4fb9d88dbe624794bf7679731e0d1cf0acd1faa728de19b0065408e`).

## 3. Сессия REA — порядок действий

1. `tools.rea.binary_session({})` — состояние сессии (open / cleanup_pending / capabilities).
2. `tools.rea.open_binary({ path })` — открыть целевой exe.
3. `tools.rea.binary_overview({})` — обзор: PE contract, Ghidra profile, memory limits.
4. `tools.rea.list_strings` / `tools.rea.search_strings({ pattern, mode })` — поиск строк
   (например: `Time left`, `Speed`, `Q`, `Queue processing started/stopped`).
5. `tools.rea.list_procedures` — разбор процедур.
6. `tools.rea.procedure_references({ address })` / `procedure_callers` — контекст вызовов.
7. `tools.rea.inspect_native_instruction` / `tools.rea.set_comment` — фиксация находок.
8. Находки → задачи в GAPS.md → реализация → тесты → статус в STATUS.md.

## 4. Каталог Awesome-REA (источник методов и кейсов)

- Репозиторий: https://github.com/reacher-z/Awesome-REA
- Сайт: https://awesomerea.org
- Machine-readable: `llms.txt`, `llms-full.txt`, `catalog.json`, API `https://awesomerea.org/api/v1/index.json`
- Категории: Website, Game, App, Native Binary, Firmware, Protocol, Agents and MCP.
- **Практика**: перед началом анализа новой области (браузерный grabber, трей, toast,
  очередь, cookie auth, scheduler) — сначала заходить в Awesome-REA по категориям и готовым
  кейсам, чтобы не изобретать велосипед и использовать проверенные методы реконструкции.

## 5. Связь с разработкой SDA

- REA-находки → задачи (`GAPS.md`) → реализация в `src/` → тесты (`cargo test`) →
  обновление `STATUS.md` → сборка/установка (`build/installer.iss`).

## 6. Текущие приоритеты реверс-анализа IDM (по GAPS.md)

1. Трей (tray) + «Start with Windows».
2. Windows toast / balloon-уведомления.
3. Browser grabber (перехват URL из браузера).
4. Cookie auth / auth-хедеры.
5. Очередь «Q» (настройки/порядок очереди).

## 7. Запуск новой REA-сессии на новом бинарнике

Для любых новых целей (другие IDM-версии, смежные приложения):
`binary_session` → `open_binary` → `binary_overview` → `list_strings`/`list_procedures`
→ при необходимости Awesome-REA для методологии.
