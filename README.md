# Pixel Hunter

Приложение для Windows 10/11: следит за цветом пикселей на экране игры и при совпадении условий выполняет действие.

Стек: Tauri 2, React 19, Redux Toolkit с RTK Query, shadcn/ui.

## Требования

- Node.js 24 и npm.
- Rust (stable) с целевой платформой MSVC.
- [Предварительные требования Tauri](https://tauri.app/start/prerequisites/) для Windows: Microsoft C++ Build Tools и WebView2.

## Установка

```powershell
npm install
```

## Запуск в режиме разработки

```powershell
npm run tauri dev
```

Debug-сборка не запрашивает права администратора и работает с правами процесса, который её запустил.

Windows не пропускает эмуляцию ввода из процесса без прав в окна процессов с правами администратора. Чтобы проверить работу на игре, запущенной от администратора, запустите терминал или IDE от имени администратора и выполните `npm run tauri dev` из него.

## Тесты

```powershell
npm test
cd src-tauri
cargo test
```

## Сборка

```powershell
npm run tauri build
```

Исполняемый файл появляется в `src-tauri/target/release/pixel-hunter.exe`, установщики — в `src-tauri/target/release/bundle/`.

Release-сборка всегда работает с правами администратора: в exe встроен манифест `requireAdministrator`, а при запуске без прав приложение само перезапускается через запрос UAC. Если запрос отклонить, приложение завершается, не открывая окно.

## Данные

Профили и отслеживания хранятся в `%APPDATA%\com.avin.pixel-hunter\config.json`. Если файл не читается, он переименовывается в `config.corrupt-<timestamp>.json`, и приложение стартует с пустой конфигурацией.
