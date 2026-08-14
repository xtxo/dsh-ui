# 告别黑框终端！我把 DeepSeek Harness 做成了 8.7MB 桌面版，已开源（Mac / Windows 双端秒开）

> **导读**：DeepSeek 刚刚开源了面向未来的 Agent 框架 **DeepSeek Harness**（“一切皆插件”）。为了让大家不用每次都在终端敲命令行、开着黑框浏览器，我基于 **tw93/Pake** 和 Rust 手搓了一个官方风格的极轻桌面客户端 **DSH-UI**。体积仅 8.7MB，双击秒开，现已全部开源并提供 Mac 与 Windows 预编译安装包！

---

## 💡 为什么想做这个？

前几天，DeepSeek 开源了备受瞩目的开发者预览版 **DeepSeek Harness**。

不得不说，这套“一切皆插件”的 Agent 架构非常惊艳，无论模型、工具、沙箱、调度还是记忆都可以随意组装。

但目前官方默认是通过命令行 `npx @deepseek-ai/dsh web` 启动一个本地 Web 服务，然后在浏览器里访问。很多朋友在使用时遇到了一些小痛点：

1. 每次想用都要打开终端手动敲命令；
2. 命令行窗口不能关，一关后台服务就挂了；
3. 浏览器标签页混在一起，容易误关；
4. 传统如果用 Electron 去打桌面壳，体积动辄 150MB~300MB，非常吃内存。

于是，我花了点时间，基于推友 **@tw93** 广受好评的轻量框架 **Pake** 以及 **Rust + Tauri**，为 DeepSeek Harness 深度定制了一款**原生极轻桌面客户端 —— DSH-UI**。

---

## 🌟 DSH-UI 有哪些核心优势？

### 1. 🍃 极致小巧：仅 8.7MB（Electron 的 1/20）
告别 Chromium 和臃肿的 Node 运行库打包。DSH-UI 直接调用系统原生的 WebView2 / WebKit 渲染容器，整个客户端安装包**仅 8.7MB**，运行内存占用减少 80% 以上，轻快如飞！

### 2. ⚡ 双击即用：后台全生命周期静默管控
这可能是最爽的一点！你**完全不需要打开任何终端**：
- 双击桌面图标，内置的 Rust 内核会自动在后台**静默拉起智能体服务**（无任何黑色 CMD 弹窗）；
- 内置毫秒级探针与握手机制，服务就绪后瞬间切入对话窗口；
- 关闭退出软件时，自动彻底释放后台进程，不留任何幽灵进程占用端口。

### 3. 🍎 苹果 Mac & 🪟 Windows 双端原生支持
- **Mac 端**：完美原生适配 Apple Silicon（M1/M2/M3/M4）以及 Intel 芯片，提供开箱即用的 `.dmg` 镜像安装包；
- **Windows 端**：支持 Windows 10 / 11 64位，提供 `.exe` 安装程序与企业级 `.msi` 安装包。

### 4. 🎨 1:1 还原 DeepSeek 品牌美学
- 专属蓝鲸高清图标（16x16 至 512x512 多尺寸适配）；
- 专属暗黑呼吸感启动过渡页；
- 支持代码与文件直接拖拽（Drag & Drop）、系统托盘常驻以及原生快捷键。

---

## 📸 客户端实机界面展示

整个客户端界面纯净沉浸，打开即是 DeepSeek 原生暗黑风格的工作区：

![DSH-UI 桌面客户端实机运行效果](https://raw.githubusercontent.com/xtxo/dsh-ui/main/assets/preview.png)

---

## 📥 如何下载使用？

项目现已发布 **v0.1.2** 版本，大家可以直接根据自己的电脑系统下载安装包体验：

### 🔗 快速下载通道
* 🍏 **Mac 用户 (Apple Silicon M系列)**：
  前往 Releases 页面下载 `DeepSeek.Harness_0.1.0_aarch64.dmg`
* 🪟 **Windows 用户 (Win 10/11)**：
  前往 Releases 页面下载 `DeepSeek.Harness_0.1.0_x64-setup.exe`

👉 **Release 官方下载页**：
[https://github.com/xtxo/dsh-ui/releases/tag/v0.1.2](https://github.com/xtxo/dsh-ui/releases/tag/v0.1.2)

👉 **项目在线展示官网 (GitHub Pages)**：
[https://xtxo.github.io/dsh-ui/](https://xtxo.github.io/dsh-ui/)

---

## 🛠️ 自己动手：如何本地编译与打包？

本项目完整开源，如果你想自行修改或二次开发打包，整个过程非常简单：

### 1. 准备环境
- 安装 Node.js (>= 18) 与 Rust 编译器 (Cargo)

### 2. 克隆仓库与依赖安装
```bash
git clone https://github.com/xtxo/dsh-ui.git
cd dsh-ui
npm install
```

### 3. 一键编译
* **Windows 编译**：
  ```powershell
  .\scripts\build-windows.ps1
  # 或 npm run build:windows
  ```
* **macOS 编译 (DMG)**：
  ```bash
  chmod +x ./scripts/build-macos.sh
  ./scripts/build-macos.sh
  ```

项目中还配置了完整的 **GitHub Actions 多平台 CI 流水线**（`.github/workflows/release.yml`），打 Tag 即可自动在云端完成 Windows、Mac 和 Linux 的交叉编译与 Release 发布！

---

## 🤝 致谢开源生态

特别致敬与鸣谢以下优秀的开源项目：
- **[tw93/Pake](https://github.com/tw93/Pake)**：极其优雅的 Rust Web 桌面化开发框架；
- **[deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness)**：DeepSeek 开源的“一切皆插件” Agent 引擎。

---

## 🎁 结语

如果你也喜欢这款只有 **8.7MB**、开箱即用的 DeepSeek Harness 桌面客户端，欢迎前往 GitHub 给个 **⭐ Star** 支持一下！

* **GitHub 开源地址**：[https://github.com/xtxo/dsh-ui](https://github.com/xtxo/dsh-ui)
* **官网主页**：[https://xtxo.github.io/dsh-ui/](https://xtxo.github.io/dsh-ui/)

大家在使用中有任何功能建议或遇到了 bug，欢迎在 GitHub Issues 留言交流！
