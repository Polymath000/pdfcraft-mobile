# PdfCraft Mobile Context & Overview

## Project Overview
PdfCraft is a clean-room, open-source, Rust-native PDF application targeting Adobe Acrobat Pro parity. Originally designed for desktop environments (macOS, Windows, Linux, FreeBSD) and WASM, this workspace (`pdfcraft-mobile`) introduces full Android mobile support with responsive and adaptive UX.

## Mobile & Android Architecture

### 1. Responsive & Adaptive UI (`crates/ui-egui`)
- **Breakpoints (`layout_traits.rs`)**:
  - Automatically identifies compact screens (`is_compact(ui)`, width < 640pt).
  - Automatically identifies mobile operating systems (`is_mobile_os()`).
  - Scales interactive elements to touch-friendly sizes (44pt minimum touch targets).
- **Adaptive Header (`chrome.rs`)**:
  - Desktop: Standard 5-mode text tabs ("All tools", "Read", "Edit", "Convert", "E-Sign") + 260pt search bar + utility buttons.
  - Mobile: Compact hamburger menu, mode dropdown selector (`[ Read ▾ ]`), direct Search and Save icon buttons, and an overflow menu (`...`) for Print, Properties, Preferences.
- **Adaptive Tab Strip (`chrome.rs`)**:
  - Omission of desktop window decorations and promotional text on mobile.
  - Retains responsive scrollable/switchable document tabs.
- **Overlay Side Panels (`lib.rs`)**:
  - Side panels (Tools, Inspector, Navigation, Bookmarks, Comments, Thumbnails) render as sliding modal overlays on mobile viewports rather than fixed horizontal splits, ensuring the document canvas is never squeezed.
- **Adaptive Home Screen (`home.rs`)**:
  - Compact horizontal margins (14pt instead of 36pt).
  - Responsive tool cards that adapt dynamically to single-column card layouts on phones.
- **Safe Android Stubs (`android_stubs.rs`)**:
  - Safe, non-panicking stubs for desktop-only dependencies (`rfd` file pickers, `arboard` clipboard) when compiling for Android, keeping code clean and portable.

### 2. Android Application (`apps/pdfcraft-android`)
- **Crate Type**: `cdylib` compiled to `libpdfcraft_android.so`.
- **Backend**: NativeActivity / `winit` 0.30 via `android-activity` with OpenGL ES (`glow`).
- **Entry Point**: `android_main(app: AndroidApp)` initializing `android_logger` and spawning `eframe::run_native`.

### 3. Building the Android APK
The project uses `cargo-apk` directly with Android NDK and SDK build tools:
```bash
./packaging/android/build-apk.sh
```
Or directly:
```bash
export ANDROID_HOME=/home/polymath/Android/Sdk
export NDK_HOME=$ANDROID_HOME/ndk/28.2.13676358
cd apps/pdfcraft-android && cargo apk build --release
```
The final installable APK is output to `dist/pdfcraft-mobile.apk`.

### 4. Native In-App Mobile File Picker (`mobile_file_picker.rs`)
- On Android and mobile touch devices, the desktop `rfd` dialogs are replaced by an in-app native `MobileFilePicker` modal:
  - Responsive directory browsing (`/sdcard/Download`, `/sdcard/Documents`, storage root, and app data).
  - Quick-access location chips for fast navigation.
  - Interactive file search & filter field.
  - Formatted file sizes and iconography (`folder`, `file-text`, `chevron-right`).
  - Directly opens files into document tabs via `app.open_path(...)` with zero blocking.
  - Declared full Android storage access permissions (`MANAGE_EXTERNAL_STORAGE`, `READ_EXTERNAL_STORAGE`, `WRITE_EXTERNAL_STORAGE`).

### 5. Testing & Verification on Android Emulator
- **Target Device**: `emulator-5554` (`sdk_gphone16k_x86_64` / Android 15/16).
- **Execution**: App installs and runs natively via Android's `NativeActivity` architecture (`ANativeActivity_onCreate`).
- **GLES Rendering**: Successfully initializes OpenGL ES 3.1 context with `egui_glow` painter.
- **Interactive Verification**:
  - Light and Dark mode toggles verified (`dist/emulator_screenshot.png`, `dist/emulator_dark_screenshot.png`).
  - Responsive tools dropdown and navigation verified (`dist/emulator_tools_screenshot.png`, `dist/emulator_all_tools_screenshot.png`).
  - Open File flow verified end-to-end:
    - Mobile File Picker modal launched from `+ Open` button (`dist/emulator_file_picker.png`).
    - File selection (`sample.pdf`) loaded into document tab and rendered to canvas (`dist/emulator_open_file_verified.png`).
  - Zero crashes or panics in logcat (`Exit code 0`, 0 errors).


