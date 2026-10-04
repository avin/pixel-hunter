# Design

## Context

Сейчас проект — шаблон Tauri 2: React 19, shadcn/ui (стиль `base-nova` на Base UI), Redux Toolkit с RTK Query поверх `invoke` (`src/store/tauriBaseQuery.ts`), `MemoryRouter` с демо-страницами и боковой навигацией. В Rust есть только команда `greet`. Тестовой инфраструктуры нет.

Это изменение закладывает данные и каркас, которые расширят следующие изменения: `add-pixel-capture` (пиксели), `add-key-sequence-action` (действие), `add-tracking-engine` (запуск и движок).

## Goals / Non-Goals

**Goals:**
- Модель данных и хранилище, которые следующие изменения расширяют без миграций.
- Единый способ вызова Rust из UI: RTK Query поверх команд Tauri.
- Повышение прав по образцу PoeLifer (`c:\_\cpp\PoeLifer\main.cpp`, `app.manifest`).

**Non-Goals:**
- Пиксели, действия, запуск отслеживаний: это следующие изменения.
- Импорт и экспорт профилей, несколько активных профилей одновременно.
- Поддержка ОС, кроме Windows 10/11.

## Decisions

### 1. Повышение прав: манифест и проверка при старте, только в release

Повторяем двухслойную схему PoeLifer:
- `build.rs` встраивает `app.manifest` с `requireAdministrator` через `tauri_build::WindowsAttributes::app_manifest`.
- `main.rs` до `tauri::Builder` проверяет членство в группе администраторов (`AllocateAndInitializeSid` и `CheckTokenMembership`). Если прав нет, перезапускает свой exe через `ShellExecuteExW` с `lpVerb = "runas"` и завершается. При отказе в UAC завершается с ненулевым кодом.

Оба слоя включаются только при `cfg(not(debug_assertions))`. В debug манифест без `requireAdministrator`, а проверка не компилируется.

Почему не повышать права и в debug: `tauri dev` следит за процессом приложения. Перезапуск через `runas` выглядит для CLI как завершение, и он останавливает dev-сервер. Для проверки на играх, запущенных от администратора, разработчик запускает терминал или IDE от администратора.

Альтернатива — только манифест без проверки при старте. Отклонена: выбрана схема PoeLifer. Проверка ничего не стоит и подстрахует, если манифест не встроился.

Собственный манифест заменяет манифест Tauri по умолчанию. Поэтому он обязан сохранить зависимость от Common Controls v6, которую добавляет `tauri-build`, иначе системные диалоги потеряют современный вид.

### 2. Конфигурацией владеет Rust, хранение — один JSON-файл

`ConfigStore` в Rust держит `Config` под `Mutex` и сохраняет его в `<app_data_dir>/config.json` после каждой мутации:
- запись во временный файл и атомарная замена через `std::fs::rename`, который на Windows заменяет существующий файл;
- при ошибке записи состояние в памяти откатывается, команда возвращает ошибку, UI показывает тост;
- если при старте `config.json` не читается, файл переименовывается в `config.corrupt-<timestamp>.json`, приложение стартует с пустой конфигурацией и сообщает об этом.

Почему Rust, а не `tauri-plugin-store` из JS: движок из `add-tracking-engine` работает в Rust и должен читать актуальную конфигурацию без обращения к WebView.

Модель (serde, `rename_all = "camelCase"`, поле `version` для будущих миграций):

```
Config  { version: 1, profiles: Vec<Profile> }
Profile { id: Uuid, name: String, processName: String, trackers: Vec<Tracker> }
Tracker { id: Uuid, name: String,
          preActionDelayMs: DelayRange { min: u32, max: u32 },   // по умолчанию 100..300
          resumeDelayMs: u32 }                                   // по умолчанию 1000
```

Следующие изменения добавляют поля трекера (`pixels`, `action`, `isRunning`) с `#[serde(default)]`, поэтому старые файлы читаются без миграции.

Правила проверки (непустые имена, `min <= max`) дублируются: в UI для показа ошибок, в Rust как последняя линия защиты.

### 3. Команды Tauri и RTK Query

Команды: `profiles_list`, `profile_create`, `profile_update`, `profile_delete`, `tracker_create`, `tracker_update`, `tracker_delete`, `windowed_processes_list`.

Мутации возвращают итоговый объект. В RTK Query теги `Profile` (список и `id`) инвалидируют список и открытый профиль. Отдельный Redux-слайс для профилей не заводим: данные из Tauri идут через RTK Query, как требует skill `redux-store`.

### 4. Список процессов с окнами

`EnumWindows` с фильтром: окно видимо, без владельца, не `WS_EX_TOOLWINDOW`, есть заголовок, не «cloaked» по `DwmGetWindowAttribute(DWMWA_CLOAKED)`. Дальше `GetWindowThreadProcessId`, `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` и `QueryFullProcessImageNameW` дают имя exe. Результат группируется по имени exe со списком заголовков, собственный процесс исключается.

Процесс в профиле сравнивается по имени exe без учёта регистра. Сравнение выполняет движок, здесь только сохраняется имя.

### 5. Каркас UI

Демо-страницы, `greet` и `AppSidebar` удаляются. Маршруты в `MemoryRouter`:
- `/` — `ProfilesPage`;
- `/profiles/:profileId` — `ProfilePage` со списком отслеживаний;
- `/profiles/:profileId/trackers/:trackerId` — `TrackerPage`.

`AppLayout` становится колонкой с шапкой (заголовок и кнопка «Назад»). Формы профиля и настроек трекера строятся на `@avinlab/react-form`, сохраняются явной кнопкой и следуют контракту ошибок из skill `react-components`.

Примитивы shadcn добавляются через CLI: `dialog`, `alert-dialog`, `table`, `select`, `combobox` (или `popover` с `command`), `sonner`.

Окно: 1100×750, минимум 900×600.

### 6. Переименование

`productName` и `title` — `Pixel Hunter`, `identifier` — `com.avin.pixel-hunter`, крейт `pixel_hunter_lib`, пакет npm `pixel-hunter`. `identifier` задаёт путь `app_data_dir`. Сменить его сейчас дешевле, чем после появления пользовательских данных.

### 7. Тесты

- Фронтенд: Vitest, jsdom, `@testing-library/react`, `@testing-library/user-event`, `@testing-library/jest-dom`. Команды Tauri подменяются через `mockIPC` из `@tauri-apps/api/mocks`: это публичная граница UI.
- Rust: `cargo test` для `ConfigStore` (временный каталог, восстановление после повреждения, откат при ошибке записи) и для проверок значений.

## Risks / Trade-offs

- [Собственный манифест теряет Common Controls v6] → манифест включает `dependentAssembly` Microsoft.Windows.Common-Controls 6.0. Проверка: в release-сборке открыть системный диалог.
- [Разработчик забыл, что dev работает без прав] → в README описано, когда запускать терминал от администратора. Ввод в игры, запущенные от администратора, без прав молча не работает.
- [Одновременная запись при быстрых мутациях] → все мутации идут через один `Mutex`, запись синхронная внутри блокировки. Объём конфигурации мал.
- [Повреждённый `config.json` после сбоя питания] → атомарная замена и переименование повреждённого файла вместо его перезаписи.
