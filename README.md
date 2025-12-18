# CompactRS
> **Native Windows Transparent Compression Tool** | **Free CompactGUI Alternative** | **Reduce Game Size** | **High Performance WOF Driver**

<div align="center">
  <img src="https://raw.githubusercontent.com/IRedDragonICY/compactrs/main/icon.ico" width="128" height="128" alt="CompactRS Icon" />
  <br />
  <br />
  
  [![Build Status](https://img.shields.io/github/actions/workflow/status/IRedDragonICY/compactrs/release.yml?branch=main&style=flat-square&logo=github&label=Build)](https://github.com/IRedDragonICY/compactrs/actions)
  [![Platform](https://img.shields.io/badge/Platform-Windows_10_%7C_11-0078D6?style=flat-square&logo=windows)](https://www.microsoft.com/windows)
  [![Language](https://img.shields.io/badge/Language-Rust-DEA584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
  [![Architecture](https://img.shields.io/badge/Architecture-x64-lightgrey?style=flat-square)](https://en.wikipedia.org/wiki/X86-64)
  [![Dependencies](https://img.shields.io/badge/Dependencies-None-success?style=flat-square)](https://github.com/IRedDragonICY/compactrs)
  [![License](https://img.shields.io/github/license/IRedDragonICY/compactrs?style=flat-square&label=License)](LICENSE)
  [![Download](https://img.shields.io/github/downloads/IRedDragonICY/compactrs/total?style=flat-square&label=Downloads&color=blue)](https://github.com/IRedDragonICY/compactrs/releases)

</div>

---

## 📸 Screenshots

<div align="center">
  <!-- Replace these with actual screenshots -->
  <img src="docs/screenshot_main.png" alt="CompactRS Main Window - Windows 11 Style" width="800" />
</div>

---

## 1. Project Overview

**CompactRS** is a specialized, high-performance file compression utility built natively for the Windows NT kernel. It serves as a modern, graphical interface for the **Windows Overlay Filter (WOF)** API, utilizing the same transparent compression technology found in Windows **"CompactOS"**.

Unlike traditional archivers (Zip, 7z, Rar), CompactRS performs **Transparent Compression**. Files processed by this utility remain **fully readable and writable** by the operating system, Games, and Explorer without requiring explicit decompression. This makes it a perfect **CompactGUI alternative** for reducing the size of:
*   **Steam / Epic Games** installations.
*   **Adobe Creative Cloud** apps.
*   **Development repositories** (node_modules, target folders).

The decompression happens on-the-fly in the kernel driver level with negligible CPU overhead.

### Zero Dependency Philosophy
This application is engineered in pure **Rust** utilizing the `windows` crate for direct Win32 API calls.
*   **No Runtime Requirements:** Does not require .NET Framework, Java, Python, or Visual C++ Redistributables.
*   **Standalone Binary:** The output is a single, static executable file (<1MB) that runs out-of-the-box on any Windows 10/11 system.
*   **Native UI:** Draws standard Windows controls via `user32.dll` and `uxtheme.dll` for a native look and feel that respects system DPI settings.

---

## 2. Technical Capabilities

### Compression Algorithms (WOF)
CompactRS exposes the internal compression formats provided by `Wof.sys`.

| Algorithm | Compression Ratio | CPU Overhead | Ideal Use Case |
| :--- | :--- | :--- | :--- |
| **XPRESS4K** | Low | Very Low | Frequently accessed system files, logs. |
| **XPRESS8K** | Medium | Low | General documents, non-media assets. |
| **XPRESS16K** | High | Medium | Applications, larger binaries. |
| **LZX** | Very High | High | Games, archival data, static software (Read-heavy). |

> **Pro Tip:** **LZX** is the gold standard for game compression, capable of reducing installation sizes by 30-60% with zero impact on loading times for most titles.

### Core Features

#### **Native Win32 Architecture**
Built directly on top of the Windows Message Loop (`GetMessage`, `DispatchMessage`). It uses a Facade pattern to wrap raw `CreateWindowExW` calls into safe Rust components, ensuring high performance and low memory footprint (~4MB RAM usage).

#### **Intelligent Batch Processing**
*   **Multithreading:** Implements a work-stealing thread pool to saturate modern multi-core CPUs during the analysis and compression phases.
*   **Safety Heuristics:** Automatically detects and skips incompressible file types (e.g., `.mp4`, `.zip`, `.jpg`) to prevent wasted CPU cycles.
*   **Lock Handling:** Integrates with the **Windows Restart Manager** API. If a file is locked by another process (e.g., a running game), CompactRS identifies the blocker and offers a prompt to terminate it cleanly before proceeding.

#### **Adaptive Visuals**
*   **Per-Monitor V2 DPI Awareness:** Crisp text and UI rendering on 4K monitors and mixed-scale setups.
*   **Immersive Dark Mode:** Uses undocumented Windows APIs (Ordinal 133/135 in `uxtheme.dll`) to apply system-consistent dark theming to legacy Win32 controls, menus, and window frames.

---

## 3. Comparison: CompactRS vs. Others

| Feature | CompactRS (WOF) | NTFS Compression (LZNT1) | 7-Zip / WinRAR |
| :--- | :--- | :--- | :--- |
| **Access Method** | Instant / Transparent | Instant / Transparent | Must Extract First |
| **Algorithm** | LZX / XPRESS (Modern) | LZNT1 (Legacy) | LZMA / LZMA2 |
| **Ratio** | High (30-60%) | Low (15-25%) | Ultra (40-70%) |
| **Performance** | Multi-threaded | Single-threaded | Multi-threaded |
| **Write Speed** | Slow (Re-compression) | Fast | N/A (Archive update) |
| **Dependencies** | None (Native) | None (Native) | Runtime / DLLs |

---

## 4. Installation

### Requirements
*   **OS:** Windows 10 (Build 17763+) or Windows 11.
*   **Privileges:** **Administrator** rights are strictly required. The WOF driver (`fsctl`) operations are privileged kernel commands.

### Download
1.  Navigate to the [Releases Page](https://github.com/IRedDragonICY/compactrs/releases).
2.  Download `compactrs.exe`.
3.  Right-click the file and select **Run as Administrator**.

---

## 5. Usage Guide

### Batch Compression
1.  **Add Target:** Drag and drop folders or files onto the application window, or use the **Files** / **Folder** buttons in the bottom action bar.
2.  **Configuration:**
    *   **Action Mode:** Select "Compress All" or "Decompress All".
    *   **Algorithm:** Select desired strength (Default: `XPRESS8K`).
        *   *Tip:* Use **LZX** for game folders to save maximum space.
    *   **Force:** Check this to force compression on files that the OS deems "not beneficial" or locked files (triggers Lock Handler).
3.  **Execute:** Click **Process All**.
4.  **Monitor:** The list view updates in real-time, showing:
    *   *Logical Size:* The actual size of the data.
    *   *Physical Size:* The size on disk after compression.
    *   *Status:* Success, Skipped, or Error details.

### Troubleshooting Locked Files
If a file is in use, a dialog will appear showing the Process Name and PID holding the lock.
*   **Force Stop:** Terminates the blocking process and retries compression immediately.
*   **Cancel:** Skips the current file.

---

## 6. Frequently Asked Questions (FAQ)

### Q: Is this safe for my files?
**A:** Yes. CompactRS uses the official Windows Overlay Filter (WOF) API, which is the same technology Windows uses for "CompactOS". It is natively supported by the kernel.

### Q: Will this slow down my games?
**A:** Generally, no. Modern CPUs (even older ones) can decompress XPRESS/LZX data faster than the disk can read it. In many cases, **loading times improve** because less data is being read from the disk.

### Q: What acts as a "CompactGUI Alternative"?
**A:** CompactRS offers similar functionality to CompactGUI but is written in Rust, has **zero dependencies** (no .NET required), and is significantly lighter/faster to start.

### Q: Can I run this on Windows 7 or 8?
**A:** No. The WOF API was introduced in Windows 10.

---

## 7. Build from Source

To compile CompactRS, you must have the **Rust Toolchain** (MSVC ABI) installed.

```powershell
# 1. Clone the repository
git clone https://github.com/IRedDragonICY/compactrs.git
cd compactrs

# 2. Build for Release
# The profile is configured for maximum size optimization (lto, strip, opt-level="z")
cargo build --release
```