# CompactRS

<div align="center">
  <img src="https://raw.githubusercontent.com/IRedDragonICY/compactrs/main/icon.ico" width="128" height="128" alt="CompactRS Icon" />
  <br />
  <br />
  
  [![Build Status](https://img.shields.io/github/actions/workflow/status/IRedDragonICY/compactrs/release.yml?branch=main&style=flat-square)](https://github.com/IRedDragonICY/compactrs/actions)
  [![Platform](https://img.shields.io/badge/platform-Windows_10_%7C_11-0078D6?style=flat-square)](https://www.microsoft.com/windows)
  [![Language](https://img.shields.io/badge/language-Rust-DEA584?style=flat-square)](https://www.rust-lang.org/)
  [![License](https://img.shields.io/github/license/IRedDragonICY/compactrs?style=flat-square)](LICENSE)
  [![Download](https://img.shields.io/github/downloads/IRedDragonICY/compactrs/total?style=flat-square)](https://github.com/IRedDragonICY/compactrs/releases)

</div>

## Overview

**CompactRS** is a high-performance, native Windows file compression utility engineered in Rust. It leverages the **Windows Overlay Filter (WOF)** API—the same technology powering the Windows "CompactOS" feature—to achieve transparent filesystem compression superior to standard NTFS (LZNT1) compression.

Unlike traditional archivers (e.g., Zip, 7z), files compressed with CompactRS remain fully readable and writable by the operating system and applications without explicit decompression. The primary objective of this project is to provide a modern, multithreaded GUI for the command-line `compact.exe` utility, offering advanced algorithms (XPRESS, LZX) to significantly reduce disk usage with negligible performance impact.

Built directly on the Win32 API using the `windows` crate, CompactRS results in an extremely lightweight binary (<1MB) with zero runtime dependencies.

## Key Features

*   **Transparent Compression:** Files remain accessible to Explorer and all applications. Decompression occurs on-the-fly via the WOF filesystem filter driver.
*   **Advanced WOF Algorithms:**
    *   **XPRESS4K:** Lowest CPU overhead, suitable for frequently accessed files.
    *   **XPRESS8K / 16K:** Balanced compression ratios.
    *   **LZX:** Maximum compression ratio (often 40-60%), ideal for cold storage, games, and large applications.
*   **High-Performance Multithreading:** utilizes a work-stealing thread pool to process batch operations efficiently, saturating modern multi-core CPUs during analysis and compression.
*   **Native Win32 Interface:** A bloat-free GUI written in pure Rust, adhering to the Windows visual style guide. Supports High DPI scaling and Per-Monitor V2 awareness.
*   **Adaptive Dark Mode:** Automatically detects system theme preferences and applies an immersive dark theme to all controls, including legacy Win32 components (ListViews, Headers, Common Controls) via undocumented UxTheme hooks.
*   **Smart Lock Handling:** Integrates with the Windows Restart Manager API to detect processes locking files, offering a built-in mechanism to terminate blockers and force compression.
*   **Safety & Filtering:** Automatically filters incompressible file types (media, existing archives) and performs intelligent "compressibility" heuristics to prevent wasted CPU cycles.

## Technical Architecture

CompactRS interfaces directly with the Windows kernel via `DeviceIoControl` ioctls.

### The WOF Mechanism
The application creates a reparse point using `FSCTL_SET_EXTERNAL_BACKING`. This instructs the filesystem to delegate read operations to the `Wof.sys` driver. The data is stored physically on the disk in a compressed format (using the selected algorithm), while the logical view remains unchanged.

### Compression Algorithms
CompactRS exposes the specific compression formats supported by the Windows ADK:

| Algorithm | CPU Cost | Compression Ratio | Use Case |
| :--- | :--- | :--- | :--- |
| **XPRESS4K** | Low | Low | System files, frequently read data. |
| **XPRESS8K** | Medium | Medium | General purpose. |
| **XPRESS16K** | Medium | High | Larger binaries. |
| **LZX** | High | Very High | Static data, games, archival storage. |

### Process Management
The application employs a custom `BatchProcess` state machine. It separates file discovery (directory walking via `ignore` crate) from processing. The worker threads utilize atomic counters for lock-free progress tracking, communicating with the main UI thread via a thread-safe message channel (`std::sync::mpsc`) to ensure the GUI remains responsive under heavy load.

## Installation

### Prerequisites
*   **OS:** Windows 10 (Build 17763+) or Windows 11.
*   **Permissions:** Administrator privileges are required to interact with the WOF driver.

### Download Binary
Download the latest pre-compiled binary from the [Releases Page](https://github.com/IRedDragonICY/compactrs/releases).

1.  Download `compactrs.exe`.
2.  Right-click the file and select **Run as Administrator**.

## Building from Source

To build CompactRS, ensure you have the latest Rust toolchain installed.

```powershell
# Clone the repository
git clone https://github.com/IRedDragonICY/compactrs.git
cd compactrs

# Build release binary
cargo build --release
```

The resulting binary will be located at `target/release/compactrs.exe`.

### Build Profile
The `Cargo.toml` is configured for maximum size optimization:
*   `opt-level = "z"`
*   `lto = true`
*   `codegen-units = 1`
*   `panic = "abort"`
*   `strip = "symbols"`

## Usage Guide

### Batch Compression
1.  Launch CompactRS.
2.  **Add Input:** Drag and drop folders onto the window, or use the **Files** / **Folder** buttons.
3.  **Select Algorithm:** Choose the desired compression strength from the dropdown (default: XPRESS8K).
    *   *Recommendation:* Use **LZX** for game folders or software directories to save the most space.
4.  **Process:** Click **Process All**.
5.  **Monitor:** The list view provides real-time statistics on logical size vs. physical size.

### Handling Locked Files
If a file is in use (e.g., a running executable or log file), CompactRS will identify the blocking process using the Windows Restart Manager. A dialog will appear offering to terminate the process to proceed with compression.

### Force Mode
By default, CompactRS skips files that do not benefit from compression. Toggle the **Force** checkbox to apply WOF compression regardless of the heuristic outcome.

## Benchmarks

Typical space savings observed on a mixed dataset (Software/Games):

*   **Uncompressed:** 45.2 GB
*   **XPRESS8K:** 28.4 GB (37% reduction)
*   **LZX:** 22.1 GB (51% reduction)

*Note: Performance impact on read speeds is negligible on modern NVMe SSDs due to reduced I/O throughput requirements offsetting the decompression CPU cost.*

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Disclaimer

This software modifies filesystem attributes at a low level. While WOF is a stable Windows feature used by the OS itself, always ensure you have backups of critical data before performing batch operations on system directories.

***

<div align="center">
  <p>Created by IRedDragonICY (Mohammad Farid Hendianto)</p>
</div>