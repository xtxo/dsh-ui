# DSH-UI: DeepSeek Harness Lightweight Desktop Client

<p align="center">
  <img src="assets/icon.png" width="128" height="128" alt="DeepSeek Harness Desktop Icon" />
</p>

<p align="center">
  <strong>Built on top of <a href="https://github.com/tw93/Pake">tw93/Pake</a> & <a href="https://github.com/deepseek-ai/deepseek-harness">deepseek-ai/deepseek-harness</a></strong>
</p>

<p align="center">
  <a href="https://github.com/xtxo/dsh-ui/releases"><img src="https://img.shields.io/github/v/release/xtxo/dsh-ui?style=flat-square&color=4d6bfe" alt="Release"></a>
  <img src="https://img.shields.io/badge/Size-8.7MB-brightgreen?style=flat-square" alt="Size">
  <img src="https://img.shields.io/badge/Platforms-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=flat-square" alt="Platforms">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-orange?style=flat-square" alt="License"></a>
</p>

<p align="center">
  <a href="https://xtxo.github.io/dsh-ui/">🌐 Live Website</a> •
  <a href="#-download">Download</a> •
  <a href="#-highlights">Highlights</a> •
  <a href="#-build-from-source">Build from Source</a> •
  <a href="README.md">中文文档</a>
</p>

---

<p align="center">
  <img src="assets/preview.png" width="900" alt="DeepSeek Harness Desktop Preview" style="border-radius: 12px; box-shadow: 0 20px 50px rgba(0,0,0,0.5);" />
</p>

---

## 🌟 Highlights

* **🍃 Ultra-Lightweight (~8.7MB)**: Free of heavy Electron overhead. Powered by Rust + native WebView2 / WebKit, consuming 80%+ less memory.
* **⚡ Sub-second Cold Starts**: Smart local cache direct locator bypasses npm/npx network query on subsequent runs.
* **🚀 Automatic Service Lifecycle**:
  * No manual CLI commands required.
  * Double-click the app; Rust kernel automatically spawns `dsh web` in silent background mode.
  * Automatic port health checks & smooth transition into the agent UI.
  * Clean process termination on exit.
* **🎨 DeepSeek Brand Design**:
  * High-res multi-size blue whale icons (16x16 to 512x512).
  * Ambient dark splash screen.
* **📁 Native Desktop Enhancements**:
  * Drag & Drop files and codebase directly into the chat window.
  * System tray resident and global shortcuts (`Ctrl+R`, `Ctrl+F`, `Alt+D`).

---

## 📥 Download

Visit [GitHub Releases (v0.1.5)](https://github.com/xtxo/dsh-ui/releases/tag/v0.1.5) or click below to download directly:

| Platform | Direct Download | Architecture |
| :--- | :--- | :--- |
| **macOS (Apple Silicon)** | 🍏 [**DeepSeek.Harness_0.1.5_aarch64.dmg**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.5/DeepSeek.Harness_0.1.5_aarch64.dmg) | M1 / M2 / M3 / M4 Macs |
| **Windows** | 🪟 [**DeepSeek.Harness_0.1.5_x64-setup.exe**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.5/DeepSeek.Harness_0.1.5_x64-setup.exe) | Windows 10 / 11 64-bit Installer |
| **Windows MSI (Chinese)** | 🪟 [**DeepSeek.Harness_0.1.5_x64_zh-CN.msi**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.5/DeepSeek.Harness_0.1.5_x64_zh-CN.msi) | MSI Chinese Installer |
| **Windows MSI (English)** | 🪟 [**DeepSeek.Harness_0.1.5_x64_en-US.msi**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.5/DeepSeek.Harness_0.1.5_x64_en-US.msi) | MSI English Installer |

---

## 🛠️ Build from Source

### 1. Prerequisites
- [Node.js](https://nodejs.org/) (>= 18)
- [Rust & Cargo](https://www.rust-lang.org/) (1.80+)

### 2. Clone & Install
```bash
git clone https://github.com/xtxo/dsh-ui.git
cd dsh-ui
npm install
```

### 3. Build

#### 🍏 macOS (DMG / APP)
```bash
chmod +x ./scripts/build-macos.sh
./scripts/build-macos.sh
# or
npm run build:mac-arm64
npm run build:mac-x64
```

#### 🪟 Windows
```powershell
.\scripts\build-windows.ps1
# or
npm run build:windows
```

#### 🐧 Linux (DEB / AppImage)
```bash
chmod +x ./scripts/build-linux.sh
./scripts/build-linux.sh
```

---

## 🤝 Acknowledgments

- **[tw93/Pake](https://github.com/tw93/Pake)**: Lightweight Rust web-to-desktop framework.
- **[deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness)**: DeepSeek's open-source agent framework.

---

## 📄 License

MIT License &copy; 2026 xtxo
