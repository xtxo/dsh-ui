# DSH-UI: DeepSeek Harness 官方风格极轻桌面客户端

<p align="center">
  <img src="assets/icon.png" width="128" height="128" alt="DeepSeek Harness Desktop Icon" />
</p>

<p align="center">
  <strong>基于 <a href="https://github.com/tw93/Pake">tw93/Pake</a> 与 <a href="https://github.com/deepseek-ai/deepseek-harness">deepseek-ai/deepseek-harness</a> 深度定制打造</strong>
</p>

<p align="center">
  <a href="https://github.com/xtxo/dsh-ui/releases"><img src="https://img.shields.io/github/v/release/xtxo/dsh-ui?style=flat-square&color=4d6bfe" alt="Release"></a>
  <img src="https://img.shields.io/badge/Size-8.7MB-brightgreen?style=flat-square" alt="Size">
  <img src="https://img.shields.io/badge/Platforms-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=flat-square" alt="Platforms">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-orange?style=flat-square" alt="License"></a>
</p>

<p align="center">
  <a href="https://xtxo.github.io/dsh-ui/">🌐 在线主页</a> •
  <a href="#-快速下载">下载客户端</a> •
  <a href="#-核心优势">核心优势</a> •
  <a href="#-本地编译与打包指南">自主构建</a> •
  <a href="README_EN.md">English</a>
</p>

---

<p align="center">
  <img src="assets/preview.png" width="900" alt="DeepSeek Harness Desktop Preview" style="border-radius: 12px; box-shadow: 0 20px 50px rgba(0,0,0,0.5);" />
</p>

---

## 🌟 核心优势

* **🍃 极致轻巧（仅 ~8.7MB）**：告别动辄 150MB+ 的臃肿 Electron 壳。基于 Rust + 系统原生 Webview2 / WebKit 渲染容器，内存占用降低 80% 以上。
* **⚡ 毫秒级极速唤醒**：内置智能缓存直启引擎，跳过 npx 的每次远端联网解析，双击即秒开。
* **🚀 全自动服务生命周期（双击即用）**：
  * 无需手动在终端输入命令。
  * 双击客户端，Rust 内核自动在后台以无黑框静默模式拉起 `dsh web` 智能体服务。
  * 自动进行端口探针与握手，就绪后瞬间切入对话窗口。
  * 退出应用时自动彻底释放后台进程，无幽灵进程残留。
* **🎨 DeepSeek 原生品牌与动效**：
  * 内置 16x16 ~ 512x512 高清多尺寸蓝鲸图标。
  * 专属暗色呼吸感启动过渡页面。
* **📁 桌面级原生交互增强**：
  * 开启本地文件 / 代码项目拖拽支持（Drag & Drop）。
  * 支持 Windows / macOS 系统托盘常驻与快捷键呼出。

---

## 📥 快速下载

前往 [GitHub Releases 页面 (v0.1.4)](https://github.com/xtxo/dsh-ui/releases/tag/v0.1.4) 或直接点击下方直链下载：

| 平台 | 安装包直接下载 | 架构说明 |
| :--- | :--- | :--- |
| **macOS (Apple Silicon)** | 🍏 [**DeepSeek.Harness_0.1.0_aarch64.dmg**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.4/DeepSeek.Harness_0.1.0_aarch64.dmg) | M1 / M2 / M3 / M4 系列 Mac |
| **Windows** | 🪟 [**DeepSeek.Harness_0.1.0_x64-setup.exe**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.4/DeepSeek.Harness_0.1.0_x64-setup.exe) | Windows 10 / 11 64位 安装包 |
| **Windows MSI (中文)** | 🪟 [**DeepSeek.Harness_0.1.0_x64_zh-CN.msi**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.4/DeepSeek.Harness_0.1.0_x64_zh-CN.msi) | MSI 中文安装包 |
| **Windows MSI (英文)** | 🪟 [**DeepSeek.Harness_0.1.0_x64_en-US.msi**](https://github.com/xtxo/dsh-ui/releases/download/v0.1.2/DeepSeek.Harness_0.1.0_x64_en-US.msi) | MSI 英文安装包 |

---

## 🛠️ 本地编译与打包指南

本项目完全开源，任何人都可以根据本指南在本地自行编译打包属于自己的客户端！

### 1. 准备环境
- 安装 [Node.js](https://nodejs.org/) (>= 18)
- 安装 [Rust 编译器与 Cargo](https://www.rust-lang.org/) (1.80+)

### 2. 克隆仓库与安装依赖
```bash
git clone https://github.com/xtxo/dsh-ui.git
cd dsh-ui
npm install
```

### 3. 一键编译

#### 🍏 macOS 编译 (DMG / APP)
```bash
# 方式一：直接运行构建脚本
chmod +x ./scripts/build-macos.sh
./scripts/build-macos.sh

# 方式二：编译 Apple Silicon 或 Intel
npm run build:mac-arm64  # M1/M2/M3/M4
npm run build:mac-x64    # Intel
```
编译产物位于：`src-tauri/target/release/bundle/dmg/`

#### 🪟 Windows 编译
```powershell
# 方式一：直接运行脚本
.\scripts\build-windows.ps1

# 方式二：使用 npm 脚本
npm run build:windows
```
编译产物位于：`src-tauri/target/release/deepseek-harness.exe`

#### 🐧 Linux 编译 (DEB / AppImage)
```bash
chmod +x ./scripts/build-linux.sh
./scripts/build-linux.sh
```

---

## 📂 项目结构

```
dsh-ui/
├── src-tauri/              # 🦀 Rust / Tauri Pake 核心源码与后台生命周期管理器
│   ├── src/
│   │   ├── app/backend.rs  # 智能后台服务探针与拉起引擎
│   │   └── lib.rs          # 客户端入口
│   ├── icons/              # 多平台高清图标 (ICNS, ICO, PNG)
│   ├── pake.json           # Pake 桌面容器配置
│   ├── tauri.conf.json     # Tauri 基础配置
│   ├── tauri.macos.conf.json # macOS 专属构建配置
│   └── tauri.windows.conf.json # Windows 专属构建配置
├── dist/                   # 🎨 内置启动过渡页 (含 DeepSeek 动效)
├── website/                # 🌐 静态展示与下载主页 (https://xtxo.github.io/dsh-ui/)
├── scripts/                # 🛠️ 跨平台构建脚本 (Windows, macOS, Linux)
└── .github/workflows/      # 🚀 GitHub Actions 多平台自动化发布流水线
```

---

## 🤝 致谢与开源生态

- **[tw93/Pake](https://github.com/tw93/Pake)**：优秀的极简 Rust Web 桌面化引擎框架。
- **[deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness)**：DeepSeek 开源的“一切皆插件” Agent 框架。

---

## 📄 开源许可证

本项目基于 [MIT License](LICENSE) 协议开源。
