<div align="center">

<img src="assets/pixora_icon.png" width="132" height="132" alt="Pixora Icon" />

# Pixora

**Next-Generation, GPU-Accelerated Screen Capture & Annotation Studio for Windows 10 & 11**

*A clean, blisteringly fast CleanShot X alternative built with pure Rust & GPUI.*

[English](#-english) • [Русский](#-русский)

[![Language](https://img.shields.io/badge/Language-Rust%202024-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![UI Framework](https://img.shields.io/badge/UI-GPUI%20(DirectX)-blueviolet.svg?style=flat-square)](https://github.com/zed-industries/zed)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011-0078D6.svg?style=flat-square&logo=windows)](https://microsoft.com)
[![Version](https://img.shields.io/badge/Version-1.2.0-8174FF.svg?style=flat-square)]()
[![Author](https://img.shields.io/badge/Author-Onzi-cyan.svg?style=flat-square)]()
[![License](https://img.shields.io/badge/License-MIT-green.svg?style=flat-square)]()

---

<img src="assets/interface.png" width="880" alt="Pixora Interface Screenshot" />

*Pixora in action: CleanShot-style floating toolbar, real-time pixel magnifier, and rich annotation canvas.*

---

</div>

<br/>

## 🌐 Language Navigation
- 🇬🇧 [English Documentation](#-english)
- 🇷🇺 [Русская документация](#-русский)

---

<br/>

<a name="english"></a>
# 🇬🇧 English

## 🚀 Overview
**Pixora** is a professional, high-performance screen capture and annotation utility engineered specifically for **Windows 10 and 11**. Inspired by the fluid elegance of macOS CleanShot X, Pixora brings pixel-accurate capture, rich vector annotations, offline OCR, and instant clipboard sharing into a lightweight, native desktop tool.

Pixora runs **100% offline and local**. Zero telemetry, zero web bloat, zero background servers.

---

## 🛠️ Built With (Tech Stack)
Pixora is architected from the ground up for extreme responsiveness, minimal memory usage, and zero runtime dependencies:

- **Language:** **Pure Rust (Edition 2024)** — memory-safe, fearless concurrency, zero garbage collection pauses.
- **UI Framework:** **[GPUI](https://github.com/zed-industries/zed)** — Zed's state-of-the-art GPU-accelerated rendering engine utilizing DirectX 11/12. Delivers buttery-smooth 60+ FPS interactions, custom vector icons, and sub-millisecond input responsiveness.
- **Platform Layer:** Direct **Win32 API** (`windows-rs` 0.61):
  - **GDI BitBlt** for instant, artifact-free virtual desktop capture across multi-monitor setups.
  - **Windows Runtime OCR (`Windows.Media.Ocr`)** for hardware-accelerated local optical character recognition.
  - **Per-Monitor V2 HiDPI Awareness** for crisp pixel-to-pixel rendering on high-resolution displays (100%–300% DPI).
  - **Shell NotifyIcon** for background system tray daemon integration.
  - **RegisterHotKey** for system-wide global shortcut capture.
- **Binary Footprint:** Single standalone executable (~12.7 MB). **Zero Electron, Zero Tauri, Zero WebView, Zero Python, Zero Node.js.** Idle RAM footprint is under 40 MB.

---

## ✨ Key Features

### 1. Precision Screen Capture & Multi-Monitor Support
- **Custom Area Capture:** Fluid, pixel-level selection with interactive dimension indicators (Width × Height).
- **Fullscreen Snapshot:** Instant capture of the entire virtual desktop spanning all displays (`Shift + PrintScreen`).
- **Active Window Capture:** One-click snapshot of the foreground focused window (`Alt + PrintScreen`).
- **True Multi-Display:** Handles negative display coordinates, mixed resolutions, and per-monitor DPI scaling without distortion.
- **Clean Capture Pipeline:** Screen bitmap is frozen *before* displaying the overlay, ensuring no capture artifacts or self-selection.

### 2. CleanShot X Aesthetic Overlay & Smart Loupe
- **Frameless Dark Glass Interface:** Obsidian dark theme (`#0B0B0D` / `#151519` / `#8174FF`) with smooth rounded corners.
- **8-Handle Interactive Resize:** Drag edges or corners with automatic coordinate normalization in any direction.
- **Pixel Loupe & Eyedropper:** Real-time magnification lens showing individual pixel grid, live coordinates (X, Y), and instant color sampling.
- **Adaptive Floating Toolbar:** Auto-centers above or below the selection with strict boundary clamping to keep controls on-screen at all times.
- **Tooltips:** Responsive hover descriptions for every tool and action.

### 3. Comprehensive Annotation Arsenal
- **Freehand Pencil:** Smooth freeform sketching with customizable line weight.
- **Marker / Highlighter:** Translucent highlighter layer (35% opacity) for document review and code callouts.
- **Arrow:** Vector arrows with clean triangular arrowheads.
- **Straight Line:** Straight guide lines at any angle.
- **Rectangle:** Outlined or solid filled bounding boxes.
- **Ellipse / Circle:** Smooth cubic Bezier curves with optional fill.
- **Text Tool:** Interactive inline text editor with Cyrillic/Russian keyboard support, backspace, and adjustable sizing (14, 18, 24 px).
- **Step Badges (Counters):** Auto-incrementing circular numbered badges (1, 2, 3...) for step-by-step tutorials.
- **Dual-Pass Gaussian Blur:** Hide confidential data with adjustable blur radius and quick presets (`Soft 6px`, `Medium 12px`, `Strong 24px`, `Heavy 36px`).
- **Mosaic Pixelate:** Mosaic block redaction with stepped sizing controls (`[-] 10px [+]`) and quick presets.
- **Stroke Width Adjustment:** Fine-grained line weight from 1 to 48 px via `[-]` / `[+]` buttons, presets, or mouse wheel.
- **Non-Destructive History:** Infinite Undo (`Ctrl + Z`) and Redo (`Ctrl + Y`) without degradation of the base image.

### 4. RGB Color Studio & Spectrum Picker
- **24-Bit Full Spectrum (16.7 Million Colors):** Pick any color imaginable.
- **360° Rainbow Hue Bar:** Smooth continuous spectrum with real-time HSV-to-RGB conversion.
- **Discrete Channel Controls:** Individual sliders and stepper buttons (`-10`, `-1`, `+1`, `+10`) for Red, Green, and Blue channels (0–255).
- **Multi-Format Color Readout:** Live HEX (`#RRGGBB`), RGB, HSV, and HSL values with direct keyboard input and 1-click **Copy HEX** button.
- **Quick Swatches:** Palette of curated designer colors for instant switching.

### 5. Hardware-Accelerated Local OCR
- Built-in offline optical character recognition powered by the native Windows Runtime engine (`Windows.Media.Ocr.OcrEngine`).
- Accurately reads Russian, English, and multilingual printed text from screenshots.
- Copies recognized text directly to the clipboard in a single click.

### 6. Seamless Export & Clipboard Sharing
- **1-Click Clipboard Copy (`Ctrl + C` / Enter):** Writes native `CF_DIB` bitmap directly to Windows clipboard. Paste instantly into Telegram, Discord, Photoshop, Slack, WhatsApp, Word, or browser.
- **Lossless PNG Export (`Ctrl + S`):** Lossless PNG compression with smart timestamp naming: `Pixora_YYYY-MM-DD_HHMMSS.png`.
- **Audio Feedback:** Subtle camera shutter sound effect upon capture.
- **Toast Notifications:** Clean feedback toasts confirm copy/save operations.

### 7. Background Daemon & Global Hotkeys
- Runs silently in the system notification tray with zero console windows.
- System tray right-click menu:
  - Capture Region (`PrintScreen`)
  - Capture Fullscreen (`Shift + PrintScreen`)
  - Capture Active Window (`Alt + PrintScreen`)
  - Settings
  - Open Screenshots Folder
  - Exit
- Global hotkey management using low-overhead Win32 hooks.

### 8. Native Settings Hub
- GPUI-powered configuration window (720 × 560 px).
- **General:** Toggle Windows startup via registry, auto-copy to clipboard, shutter sounds, and screenshot destination folder.
- **Capture:** Configure loupe visibility, dimension badges, and default brush stroke.
- **Shortcuts:** **Interactive hotkey remapping** directly from the UI with real-time Windows API re-registration.
- **About:** Version information, architecture overview, and credits.

---

## ⌨️ Default Hotkeys

| Shortcut | Description | Configurable |
|---|---|:---:|
| `PrintScreen` | Capture selected region | Yes |
| `Shift + PrintScreen` | Capture entire virtual screen | Yes |
| `Alt + PrintScreen` | Capture current active window | Yes |
| `Ctrl + C` / `Enter` | Copy screenshot to clipboard | No |
| `Ctrl + S` | Save screenshot to PNG file | No |
| `Ctrl + Z` | Undo last action | No |
| `Ctrl + Y` | Redo action | No |
| `Escape` | Cancel selection / Close overlay | No |
| `Mouse Wheel` | Increase / decrease brush stroke width | No |

---

## 📦 Building from Source

### Prerequisites
- Windows 10 or 11 (x64)
- Rust toolchain (version 1.82+ recommended)

### Build Commands
```bash
# Clone the repository
git clone https://github.com/Onzis/PIXORA.git
cd PIXORA

# Check compilation and type-check
cargo check

# Run automated test suite (12 unit & integration tests)
cargo test

# Compile release binary with embedded Windows icon & metadata
cargo build --release
```

The resulting standalone executable is generated at:
```text
target/release/pixora.exe
```

---

<br/>

<a name="русский"></a>
# 🇷🇺 Русский

## 🚀 О проекте
**Pixora** — профессиональная, сверхбыстрая утилита для создания скриншотов и аннотаций, созданная специально для **Windows 10 и 11**. Вдохновленная легкостью и эстетикой CleanShot X на macOS, Pixora объединяет пиксельную точность, богатый набор векторных инструментов рисования, локальный OCR и мгновенную работу с буфером обмена в компактном нативном приложении.

Pixora работает **на 100% локально и автономно**. Без облачных серверов, скрытой телеметрии, веб-надстроек и отправки данных в сеть.

---

## 🛠️ На чем написана (Технологический стек)
Архитектура Pixora спроектирована для максимальной производительности, мгновенного отклика и минимального потребления памяти:

- **Язык разработки:** **Чистый Rust (Edition 2024)** — безопасность памяти, отсутствие сборок мусора (GC), высочайшая скорость компилированного машинного кода.
- **Графический UI-фреймворк:** **[GPUI](https://github.com/zed-industries/zed)** — гибридный GPU-ускоренный движок рендеринга редактора Zed, работающий через DirectX 11/12. Обеспечивает стабильные 60+ FPS, плавные анимации, векторную отрисовку и отклик на действия мыши с минимальной задержкой.
- **Системный слой Windows:** Прямой **Win32 API** (`windows-rs` 0.61):
  - **GDI BitBlt** — моментальный захват виртуального рабочего стола без цветовых искажений и артефактов на любых конфигурациях мониторов.
  - **Windows Runtime OCR (`Windows.Media.Ocr`)** — аппаратное оптическое распознавание печатного текста прямо на устройстве.
  - **Per-Monitor V2 HiDPI** — идеальная резкость и соответствие пикселей при масштабировании от 100% до 300%.
  - **Shell NotifyIcon** — бесшовный фоновый демон в системном трее Windows.
  - **RegisterHotKey** — глобальный перехват горячих клавиш в отдельном потоке.
- **Размер и ресурсы:** Единый автономный `.exe` файл (~12.7 МБ). **Полный отказ от Electron, Tauri, WebView, Chromium, Python и Node.js.** Потребление оперативной памяти в фоне — менее 40 МБ.

---

## ✨ Ключевые возможности

### 1. Точный захват экрана и поддержка нескольких дисплеев
- **Захват произвольной области:** Интерактивное выделение мышью с выводом актуальных размеров (Ш × В в пикселях).
- **Полноэкранный снимок:** Мгновенный снимок всего виртуального экрана со всеми подключенными дисплеями (`Shift + PrintScreen`).
- **Снимок активного окна:** Фиксация окна, находящегося в фокусе (`Alt + PrintScreen`).
- **Мультимониторность:** Корректная поддержка мониторов с разным разрешением, ориентацией и отрицательными координатами.
- **Чистый захват:** Снимок экрана фиксируется *до* показа интерфейса, исключая попадание элементов управления на готовое изображение.

### 2. Дизайн CleanShot X и пиксельная лупа
- **Темная стеклянная эстетика:** Премиальная палитра (`#0B0B0D` / `#151519` / `#8174FF`) без стандартных оконных рамок Windows.
- **8 интерактивных маркеров:** Изменение размера за любую сторону или угол с автоматической нормализацией координат.
- **Лупа (Magnifier) и Пипетка:** Увеличенная сетка пикселей под курсором с координатами (X, Y) и моментальным определением цвета.
- **Адаптивная плавающая панель:** Центрируется относительно рамки выделения, размещается сверху или снизу и надежно удерживается в границах экрана.
- **Всплывающие подсказки:** Подробные описания для всех кнопок при наведении.

### 3. Богатый арсенал аннотаций
- **Карандаш (Pencil):** Плавное рисование произвольных линий с выбранной толщиной.
- **Маркер / Хайлайтер (Marker):** Полупрозрачный слой (35% непрозрачности) для выделения строк кода и текста.
- **Стрелка (Arrow):** Векторная стрелка с четким треугольным наконечником.
- **Прямая линия (Line):** Точные направляющие под любым углом.
- **Прямоугольник (Rectangle):** Контурные и сплошные прямоугольники.
- **Эллипс / Круг (Ellipse):** Идеально сглаженные эллипсы на кубических кривых Безье.
- **Текст (Text):** Ввод текста прямо на холсте с поддержкой кириллицы/русской раскладки, клавиши Backspace и переключением размера шрифта (14, 18, 24 px).
- **Нумерованные шаги (Step Badges):** Последовательные метки (1, 2, 3...) с автоматическим инкрементом номера при каждом клике.
- **Размытие по Гауссу (Blur):** Двухпроходное блюр-сглаживание конфиденциальных данных с пошаговой регулировкой (`[-] 12px [+]`) и пресетами (`Soft 6px`, `Medium 12px`, `Strong 24px`, `Heavy 36px`).
- **Пикселизация (Pixelate):** Мозаичное скрытие областей с настройкой размера блоков (`[-] 10px [+]`).
- **Настройка толщины штриха:** От 1 до 48 px с помощью кнопок `[-]` / `[+]`, пресетов или колесика мыши.
- **Неразрушающая история:** Неограниченные отмена (`Ctrl + Z`) и повтор (`Ctrl + Y`) без деградации исходного снимка.

### 4. Профессиональная палитра RGB Color Studio
- **Полный спектр 24-bit RGB (16.7 млн цветов):** Выбор любого существующего оттенка.
- **360° радужная шкала Hue:** Плавный спектр тонов с конвертацией HSV ↔ RGB.
- **Раздельные регуляторы каналов R, G, B:** Кнопки быстрой подстройки (`-10`, `-1`, `+1`, `+10`) и числовые поля (0–255).
- **Поддержка форматов цвета:** Отображение HEX (`#RRGGBB`), RGB, HSV, HSL, прямой ввод HEX с клавиатуры и кнопка быстрого копирования цвета.
- **Быстрые цвета:** Палитра эталонных дизайнерских оттенков для мгновенного выбора.

### 5. Локальный OCR (Распознавание текста)
- Встроенное оптическое распознавание на базе нативного движка `Windows.Media.Ocr.OcrEngine`.
- Распознает русский, английский и смешанный текст без интернета.
- Скопированный текст сразу помещается в буфер обмена.

### 6. Экспорт и мгновенная вставка
- **Копирование в буфер (`Ctrl + C` / Enter):** Запись нативного Windows-формата `CF_DIB`. Изображение со всеми аннотациями вставляется в Telegram, Discord, Photoshop, браузеры и офисные пакеты нажатием `Ctrl + V`.
- **Сохранение в PNG (`Ctrl + S`):** Беспотерьное сжатие PNG с автоматической генерацией имени по времени: `Pixora_YYYY-MM-DD_HHMMSS.png`.
- **Звук затвора:** Приятный ненавязчивый щелчок камеры при успешном захвате.
- **Всплывающие уведомления (Toasts):** Информативные CleanShot-уведомления об операциях.

### 7. Системный трей и глобальные горячие клавиши
- Работа в фоновом режиме без открытия консольных окон.
- Меню в системном трее Windows:
  - Сделать скриншот (`PrintScreen`)
  - Снимок всего экрана (`Shift + PrintScreen`)
  - Снимок окна (`Alt + PrintScreen`)
  - Настройки
  - Открыть папку скриншотов
  - Выход
- Надежные глобальные хоткеи через Win32 API.

### 8. Окно настроек (Settings Hub)
- Нативное окно на GPUI (720 × 560 px).
- **Раздел General:** Автозапуск с Windows через системный реестр, автокопирование в буфер, звуковые эффекты, выбор папки сохранения и открытие в Проводнике.
- **Раздел Capture:** Настройка лупы, отображения размеров и базовой толщины линий.
- **Раздел Shortcuts:** **Интерактивное переназначение горячих клавиш** прямо из интерфейса с немедленной перерегистрацией в Windows.
- **Раздел About:** Версия программы, сведения об архитектуре и авторстве.

---

## ⌨️ Таблица горячих клавиш

| Сочетание | Описание действия | Настраивается |
|---|---|:---:|
| `PrintScreen` | Захват произвольной области экрана | Да |
| `Shift + PrintScreen` | Мгновенный снимок всего виртуального экрана | Да |
| `Alt + PrintScreen` | Снимок текущего активного окна | Да |
| `Ctrl + C` / `Enter` | Скопировать готовый скриншот в буфер обмена | Нет |
| `Ctrl + S` | Сохранить скриншот в файл PNG | Нет |
| `Ctrl + Z` | Отменить последнее действие (Undo) | Нет |
| `Ctrl + Y` | Повторить отмененное действие (Redo) | Нет |
| `Escape` | Сбросить выделение / выйти из оверлея | Нет |
| `Колесико мыши` | Изменить толщину текущего инструмента | Нет |

---

## 📦 Сборка из исходников

### Системные требования
- 64-битная операционная система Windows 10 или Windows 11.
- Установленный компилятор Rust (версия 1.82 или новее).

### Инструкция по сборке
```bash
# Клонирование репозитория
git clone https://github.com/Onzis/PIXORA.git
cd PIXORA

# Проверка типов и кода
cargo check

# Запуск набора тестов
cargo test

# Сборка оптимизированного релиза с встроенными ресурсами и иконкой
cargo build --release
```

Готовый бинарный файл создается по пути:
```text
target/release/pixora.exe
```

---

## 👤 Автор и лицензия
- **Автор:** **Onzi**
- **Проект:** **Pixora**
- **Лицензия:** [MIT License](LICENSE)
