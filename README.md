# TTPlayer-Next

<p align="center">
  <strong>一款基于 Tauri 2 + Rust + React 19 的现代化跨平台桌面音乐播放器</strong>
</p>

<p align="center">
  <img alt="Version" src="https://img.shields.io/badge/version-0.2.1-blue">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.96+-orange">
  <img alt="React" src="https://img.shields.io/badge/React-19-black">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-purple">
  <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/ttplayer-next/ttplayer-next/ci.yml?label=CI">
</p>

---

## 目录

- [项目概述](#项目概述)
- [核心功能](#核心功能)
- [技术栈](#技术栈)
- [项目结构](#项目结构)
- [环境要求](#环境要求)
- [快速开始](#快速开始)
- [使用指南](#使用指南)
- [命令 API 参考](#命令-api-参考)
- [开发指南](#开发指南)
- [测试](#测试)
- [贡献规范](#贡献规范)
- [版本变更记录](#版本变更记录)
- [许可证](#许可证)
- [联系方式](#联系方式)

---

## 项目概述

TTPlayer-Next 是一款高性能、轻量级的跨平台桌面音乐播放器，采用 Tauri 2 框架构建，前端使用 React 19 + TypeScript，后端音频引擎完全由 Rust 实现。它支持多种音频格式、实时频谱可视化、歌词同步、桌面歌词、均衡器、皮肤系统等丰富功能，目标是提供 Winamp 时代的精简体验同时融入现代 UI 与跨平台能力。

- 跨平台：Windows / macOS / Linux
- 原生性能：Rust 实现的音频解码、DSP 与输出
- 小体积：最终打包体积远小于 Electron 方案
- 可定制：支持皮肤系统、主题切换、桌面歌词样式自定义

## 核心功能

### 播放与音频

- 多格式解码：MP3、FLAC、WAV、AAC、ALAC、Vorbis、OGG、AC3、APE、XM/Module 音乐
- 播放控制：播放 / 暂停 / 停止 / 上一首 / 下一首 / 跳转进度
- 音量控制与静音
- 播放模式：顺序、循环、随机、单曲循环
- 交叉淡入淡出（Crossfade）：余弦渐变过渡 + ring buffer drain 保证平滑切换
- 环绕声宽度调节
- 实时频谱分析器（256 频段下采样为 64 频段，20fps 推送）

### 均衡器

- 多频段均衡器
- 前置增益（Preamp）调节
- 一键重置

### 歌词

- 本地歌词加载（支持多种格式）
- 在线歌词搜索与下载
- 自动匹配歌词
- 歌词保存到文件
- 桌面歌词悬浮窗（独立窗口，支持字体、颜色、大小自定义）
- 卡拉OK式逐行高亮

### 播放列表

- 添加文件 / 文件夹
- 拖放排序
- 删除 / 清空
- 自动持久化（防抖写入）

### 元数据与标签

- 读取音频文件属性（比特率、采样率、时长等）
- 标签编辑（ID3v2、FLAC、APE 等）
- 批量标签编辑：多选文件 + 独立窗口 + 字段级应用开关 + 覆盖/仅填充空字段双模式
- 文件属性对话框

### 格式转换

- 音频格式转换
- 多格式输出支持

### 个性化

- 皮肤系统：安装、切换、管理自定义皮肤（.ttskin 包）
- 主题模式：亮色 / 暗色
- 迷你模式
- 全局热键支持

### 系统集成

- 系统托盘图标
- 全局快捷键
- 优雅退出（刷新未保存的播放列表）

## 技术栈

### 前端

| 技术 | 版本 | 用途 |
|------|------|------|
| [React](https://react.dev) | 19 | UI 框架 |
| [TypeScript](https://www.typescriptlang.org) | 5.7 | 类型安全 |
| [Vite](https://vitejs.dev) | 6 | 构建工具与开发服务器 |
| [Zustand](https://github.com/pmndrs/zustand) | 5 | 状态管理 |
| [Vitest](https://vitest.dev) | 3 | 单元测试 |
| [Playwright](https://playwright.dev) | 1.40+ | 端到端测试 |
| [Biome](https://biomejs.dev) | - | Lint 与格式化 |

### 后端（Rust）

| 技术 | 版本 | 用途 |
|------|------|------|
| [Tauri](https://tauri.app) | 2 | 桌面应用框架 |
| Rust Edition | 2024 | - |
| [Symphonia](https://github.com/pdeljanov/Symphonia) | 0.6 | 音频解码（AAC/FLAC/MP3/Vorbis/WAV 等） |
| [cpal](https://github.com/RustAudio/cpal) | 0.15 | 跨平台音频输出 |
| [rustfft](https://github.com/mehcode/rustfft) | 6 | FFT 频谱分析 |
| [rubato](https://github.com/HEnquist/rubato) | 0.14 | 音频重采样 |
| [lofty](https://github.com/Serial-ATA/lofty-rs) | 0.21 | 音频标签读写 |
| [xmrs](https://github.com/jrmuizel/xmrs) | 0.14 | Module 音乐格式 |
| [ape-decoder](https://crates.io/crates/ape-decoder) | 0.3 | APE 格式解码 |
| [oxideav-ac3](https://crates.io/crates/oxideav-ac3) | 0.0.9 | AC3 解码 |
| [reqwest](https://github.com/seanmonstar/reqwest) | 0.12 | HTTP 请求（在线歌词） |
| [tokio](https://tokio.rs) | 1 | 异步运行时 |
| [parking_lot](https://github.com/Amanieu/parking_lot) | 0.12 | 高性能同步原语 |
| [tracing](https://github.com/tokio-rs/tracing) | 0.1 | 结构化日志 |
| [criterion](https://github.com/bheisler/criterion.rs) | 0.5 | 性能基准测试 |

## 项目结构

```
ttplayer-next/
├── src/                          # React 前端
│   ├── components/
│   │   ├── MainPanel/            # 主面板（播放控制、频谱、歌词、播放列表）
│   │   │   ├── Equalizer.tsx
│   │   │   ├── LyricsPanel.tsx
│   │   │   ├── Spectrum.tsx
│   │   │   └── MainPanel.tsx
│   │   ├── SettingsPanel.tsx     # 设置面板
│   │   ├── TagEditor.tsx         # 标签编辑器
│   │   ├── BatchTagEditor.tsx    # 批量标签编辑器
│   │   ├── FormatConverter.tsx   # 格式转换器
│   │   ├── FilePropertiesDialog.tsx
│   │   ├── MiniMode.tsx          # 迷你模式
│   │   └── SkinSelector.tsx     # 皮肤选择器
│   ├── hooks/                    # React Hooks
│   ├── stores/                   # Zustand 状态管理
│   ├── skins/                    # 内置皮肤资源
│   ├── utils/                    # 工具函数（IPC 封装等）
│   ├── batch-editor.tsx          # 批量编辑独立窗口入口
│   ├── batch-editor.html         # 批量编辑独立页面
│   └── App.tsx
├── src-tauri/                    # Tauri 应用（Rust）
│   ├── src/
│   │   ├── commands/             # Tauri 命令处理器
│   │   │   ├── player.rs
│   │   │   ├── playlist.rs
│   │   │   ├── lyrics.rs
│   │   │   ├── tags.rs
│   │   │   ├── convert.rs
│   │   │   ├── skin.rs
│   │   │   ├── desktop_lyrics.rs
│   │   │   └── ...
│   │   ├── hotkeys.rs            # 全局快捷键
│   │   ├── tray.rs               # 系统托盘
│   │   └── state.rs              # 应用状态
│   └── tauri.conf.json           # Tauri 配置
├── crates/                       # Rust 工作区子项目
│   ├── tt-common/                # 通用类型与常量
│   ├── tt-core/                  # 音频核心引擎
│   │   └── src/
│   │       ├── codecs/           # 编解码器适配
│   │       ├── dsp/              # 数字信号处理（EQ、淡入淡出、频谱、环绕）
│   │       ├── lyrics/           # 歌词解析与同步
│   │       └── player/           # 播放器传输层
│   ├── tt-tags/                  # 标签读写
│   └── tt-playlist/              # 播放列表管理
├── scripts/                      # 构建辅助脚本
│   └── sync-version.mjs          # 集中式版本同步
├── e2e/                          # Playwright 端到端测试
├── .github/workflows/            # CI 配置
├── version.json                  # 单一版本源
└── Cargo.toml                    # Rust 工作区根配置
```

## 环境要求

### 通用要求

- [Node.js](https://nodejs.org/) >= 20
- [npm](https://www.npmjs.com/) >= 10（或 pnpm / yarn）
- [Rust](https://www.rust-lang.org/tools/install) >= 1.96（Edition 2024）

### 平台特定依赖

**Windows**
- MSVC 构建工具（Visual Studio Build Tools）
- WebView2（Windows 10/11 通常已预装）

**macOS**
- Xcode Command Line Tools
```bash
xcode-select --install
```

**Linux**（Debian/Ubuntu）
```bash
sudo apt-get install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev pkg-config
```

## 快速开始

### 1. 克隆仓库

```bash
git clone https://github.com/ttplayer-next/ttplayer-next.git
cd ttplayer-next
```

### 2. 安装前端依赖

```bash
npm install
```

### 3. 开发模式运行

```bash
npm run tauri dev
```

此命令会同时启动 Vite 开发服务器（`http://localhost:5173`）和 Tauri 后端开发构建，首次运行会编译 Rust 依赖，耗时较长。

### 4. 构建生产版本

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`，包含对应平台的安装包。

## 使用指南

### 基本操作

1. **打开文件**：点击「添加」按钮选择音频文件，或直接拖放文件到窗口
2. **播放控制**：使用底部播放栏的播放/暂停/上一首/下一首按钮
3. **音量**：拖动音量滑块调节
4. **进度跳转**：点击进度条任意位置跳转

### 歌词

- 加载本地歌词：在歌词面板右键选择「加载歌词文件」
- 在线搜索：选择「在线搜索歌词」并从结果中选择
- 桌面歌词：在设置中开启桌面歌词悬浮窗

### 均衡器

- 点击工具栏的均衡器图标打开
- 调节各频段增益
- 可调整前置增益
- 点击「重置」恢复默认

### 皮肤

- 点击皮肤选择器图标
- 选择已安装的皮肤
- 安装新皮肤：选择 `.ttskin` 文件

### 迷你模式

- 切换至迷你模式以节省桌面空间
- 再次点击恢复完整界面

### 全局快捷键

应用支持全局媒体快捷键，可在系统托盘菜单中查看与管理。

## 命令 API 参考

TTPlayer-Next 通过 Tauri 的 IPC 机制暴露了一系列命令，前端通过 `@tauri-apps/api` 的 `invoke` 调用。以下是主要命令分组：

### 播放控制

| 命令 | 说明 |
|------|------|
| `player_play` | 播放当前曲目 |
| `player_pause` | 暂停 |
| `player_stop` | 停止 |
| `player_toggle` | 播放/暂停切换 |
| `player_get_state` | 获取播放状态 |
| `player_seek` | 跳转到指定位置（毫秒） |
| `player_set_volume` | 设置音量（0-100） |

### 均衡器

| 命令 | 说明 |
|------|------|
| `eq_get_bands` | 获取各频段增益 |
| `eq_set_band` | 设置指定频段增益 |
| `eq_get_preamp` | 获取前置增益 |
| `eq_set_preamp` | 设置前置增益 |
| `eq_reset` | 重置均衡器 |

### 播放列表

| 命令 | 说明 |
|------|------|
| `playlist_add_files` | 添加文件 |
| `playlist_add_folder` | 添加文件夹 |
| `playlist_get_items` | 获取列表项 |
| `playlist_next` / `playlist_prev` | 上一首 / 下一首 |
| `playlist_play_index` | 播放指定索引 |
| `playlist_clear` | 清空列表 |
| `playlist_remove` | 移除指定项 |
| `playlist_move_item` | 移动列表项 |
| `playlist_get_play_mode` / `playlist_set_play_mode` | 获取/设置播放模式 |

### 歌词

| 命令 | 说明 |
|------|------|
| `lyrics_load` | 加载本地歌词 |
| `lyrics_search` | 搜索歌词 |
| `lyrics_auto_load` | 自动匹配加载 |
| `lyrics_search_online` | 在线搜索 |
| `lyrics_load_online` | 加载在线歌词 |
| `lyrics_save_to_file` | 保存歌词到文件 |
| `lyrics_get_lines` | 获取歌词行 |
| `lyrics_get_servers` / `lyrics_set_servers` | 获取/设置歌词服务器 |

### 标签与文件

| 命令 | 说明 |
|------|------|
| `tags_read` / `tags_write` | 读取/写入标签 |
| `tags_read_batch` / `tags_write_batch` | 批量读取/写入标签 |
| `file_get_properties` | 获取文件属性 |

### 格式转换

| 命令 | 说明 |
|------|------|
| `convert_files` | 转换音频文件 |
| `convert_get_formats` | 获取支持的输出格式 |

### 皮肤与主题

| 命令 | 说明 |
|------|------|
| `skin_list` | 列出已安装皮肤 |
| `skin_apply` | 应用皮肤 |
| `skin_install` | 安装皮肤 |
| `skin_delete` | 删除皮肤 |
| `theme_get_mode` / `theme_set_mode` | 获取/设置主题 |

### 桌面歌词

| 命令 | 说明 |
|------|------|
| `desktop_lyrics_get` | 获取桌面歌词设置 |
| `desktop_lyrics_set` | 更新桌面歌词设置 |
| `desktop_lyrics_reset` | 重置为默认 |

### 事件

后端通过 `app_handle.emit` 推送事件，前端使用 `listen` 监听：

| 事件 | 说明 |
|------|------|
| `player-state-update` | 播放状态更新（约 20fps，含位置、音量、频谱、歌词） |
| `desktop-lyrics-settings-changed` | 桌面歌词设置变更（跨窗口同步） |

> 注：`desktop_lyrics_set` 命令参数使用 camelCase 键名（`fontSize`、`fontFamily`、`fontColor`），Tauri 2 会自动转换为 Rust 的 snake_case 参数。

## 开发指南

### 常用脚本

```bash
# 开发
npm run tauri dev

# 前端开发服务器（仅 UI）
npm run dev

# 类型检查 + 构建
npm run build

# Lint
npm run lint

# 格式化
npm run format

# 版本同步（单一版本源 → 所有消费方）
npm run sync-version
```

### Rust 开发

```bash
# 检查
cargo check

# 运行测试（库 crate）
cargo test -p tt-common -p tt-core -p tt-tags -p tt-playlist

# 性能基准测试
cargo bench -p tt-core

# Clippy 检查
cargo clippy --all-targets

# 格式化
cargo fmt --all
```

### 架构说明

- **事件推送线程**：后端在 `setup` 中启动一个后台线程，每 50ms（20fps）向前端推送 `player-state-update` 事件，包含播放状态、位置、音量、频谱（下采样 256→64 频段）、歌词同步信息。元数据（含 base64 封面图）仅在文件切换或标签异步读取完成时推送，避免每帧传输大体积数据。
- **状态管理**：后端使用 `Arc<Mutex<...>>` 与 `parking_lot` 管理共享状态；前端使用 Zustand store 镜像后端状态。
- **DSP 子系统**：均衡器、交叉淡入淡出、频谱分析、环绕声等均以独立子模块实现，使用各自的细粒度锁，避免阻塞传输层。
- **歌词引擎**：随事件推送线程同步更新当前行索引与进度，前端无需单独轮询。

### 版本管理

项目采用集中式版本管理：`version.json` 是唯一版本源，修改后运行 `npm run sync-version` 即可同步至所有消费方。每次 `npm run dev` / `npm run build` 时通过 `predev` / `prebuild` 钩子自动同步，无需手动操作。

| 消费方 | 方式 |
|--------|------|
| `Cargo.toml` `[workspace.package]` | 5 个 Rust crate 通过 `version.workspace = true` 继承 |
| `package.json` | npm 包版本 |
| `src-tauri/tauri.conf.json` | Tauri exe 元数据 |
| `src/version.ts`（自动生成） | 前端 `import { APP_VERSION } from '@/version'` |
| Rust 编译时 | `build.rs` 注入 `APP_VERSION` 环境变量 → `env!("APP_VERSION")` |

> `src/version.ts` 由同步脚本自动生成，已加入 `.gitignore`，无需提交。

### 编码规范

- 前端使用 Biome 进行 lint 与格式化：`npm run lint` / `npm run format`
- Rust 使用 rustfmt 与 clippy：`cargo fmt --all -- --check` / `cargo clippy`
- UI 文本元素：主文本透明度 >= 0.7，次文本透明度 >= 0.5，确保可读性
- 播放符号（播放/暂停）使用 `viewBox="0 0 24 24"` 的 SVG 图标，避免 Unicode 字符导致的对齐问题
- 桌面歌词字体大小范围 12-48px，字体颜色使用 `#RRGGBB` 格式

## 测试

### 前端单元测试

```bash
npm run test          # 运行一次
npm run test:watch    # 监听模式
npm run test:ui       # UI 界面
```

### 端到端测试

```bash
npm run e2e           # 运行 Playwright 测试
npm run e2e:ui        # UI 界面
```

### Rust 测试

```bash
cargo test -p tt-common -p tt-core -p tt-tags -p tt-playlist
```

## 贡献规范

欢迎提交 Issue 与 Pull Request！请遵循以下流程：

1. **Fork** 本仓库
2. 创建特性分支：`git checkout -b feature/your-feature`
3. 提交更改：请使用清晰的提交信息，说明「为什么」而不仅是「做了什么」
4. 确保通过所有检查：
   ```bash
   npm run lint && npm run test && npm run build
   cargo fmt --all -- --check && cargo clippy --all-targets
   cargo test -p tt-common -p tt-core -p tt-tags -p tt-playlist
   ```
5. 提交 Pull Request 到 `main` 分支，并描述变更内容与动机

### 提交信息约定

建议使用约定式提交（Conventional Commits）格式：

```
<type>(<scope>): <subject>

<body>
```

常用 type：`feat`（新功能）、`fix`（修复）、`refactor`（重构）、`docs`（文档）、`test`（测试）、`chore`（构建/工具）。

## 版本变更记录

### v0.2.2

本次发布聚焦于 Crossfade 淡入淡出过渡可靠性修复、批量标签编辑与集中式版本管理。

#### 新增功能

- **批量标签编辑**：播放列表新增「多选」模式，可同时选中多个文件进行标签编辑。支持字段级应用开关（仅修改勾选的字段）、两种写入模式（覆盖写入 / 仅填充空字段）、独立编辑窗口（皮肤/主题与主窗口实时同步）。后端使用 `lofty` 的原子临时文件策略保证写入安全

#### 修复的问题

- **Crossfade 切歌提前截断**：自动切歌时前一首歌曲被突然切断、淡入淡出效果完全无声。根因为 crossfade 混音完成后立即设置 `Stopped` 状态，前端随即调用 `playNext()` → `stop_inner()` flush 环形缓冲（10 秒容量），导致尚未播放的淡出尾部数据被全部丢弃。修复方案：混音完成后先等待 ring buffer 被 output callback 完全 drain（`read_pos` 追上 `write_pos`），再向事件推送线程发送 `Stopped`，确保淡出混音数据完整播放后再触发切歌。Drain 等待含三重保护：ring 排空检测、状态变更中止（手动切歌不阻塞）、自适应超时（缓冲时长 + 3 秒余量）
- **Crossfade 完成信号竞态**：修复 `crossfade_pending` 与 `state=Stopped` 两个原子变量写入顺序不当导致的竞态窗口。原顺序为先清 `crossfade_pending` 再设 `Stopped`，事件推送线程在两个写入间隙可能观察到 `crossfadePending=false` + `state=Playing`，错误清除 `crossfadeActiveRef`，导致随后收到的 `Stopped` 不再触发 `playNext`，播放卡住。修正为先设 `Stopped` 再清 `crossfade_pending`
- **关于面板版本号过时**：SettingsPanel 中硬编码版本 `0.1.0`，与实际 `0.2.1` 不一致。改为动态导入 `src/version.ts` 的 `APP_VERSION` 常量

#### 改进

- **集中式版本管理**：新增 `version.json` 作为项目唯一版本源，修改一处即可全局生效；创建 `scripts/sync-version.mjs` 同步脚本自动传播至 `Cargo.toml`、`package.json`、`tauri.conf.json`、`src/version.ts`；集成 `predev` / `prebuild` npm 钩子实现开发/构建时自动同步；`build.rs` 编译时注入 `APP_VERSION` 环境变量供 Rust 代码使用
- **PlaybackRing 新增 `wait_until_drained` 方法**：基于 `read_notify`（`advance_read` 信号）的异步等待，驱动 crossfade drain 循环，零 CPU 轮询

#### 重大变更

无

---

### v0.2.1

本次发布为修复补丁版本，聚焦于桌面歌词皮肤同步、CSP 安全策略、LRC 歌词编码识别及 Windows 构建稳定性。

#### 新增功能

- **LRC 传统编码自动识别**：LRC 歌词文件现支持自动检测并转换 GBK / Big5 / Shift-JIS 等传统编码至 UTF-8，匹配国内主流音乐应用的解码行为，解决中文歌词乱码问题。解码顺序：UTF-8 BOM → 原生 UTF-8 → `chardetng` 编码检测兜底
- **LRC 文件名大小写不敏感匹配**：`search_lrc_files` 改为大小写不敏感匹配 `.lrc` 扩展名与文件名（如 `Song.LRC` 可匹配 `song.mp3`），适配 Windows 大小写不敏感文件系统及用户下载习惯
- **NSIS 安装包简体中文**：Windows 安装包添加简体中文语言支持

#### 修复的问题

- **桌面歌词皮肤切换不同步**：修复桌面歌词窗口完全不跟随主窗口皮肤切换的问题。根因为多重故障叠加：
  1. `emitTo('lyrics-desktop', 'skin-changed')` 静默失败（`.catch(() => {})`）且无重试 → 添加 3 次重试 + 指数退避（80ms/160ms）
  2. Tauri 2 dev 模式下 Vite 注入 CSP nonce 导致 `'unsafe-inline'` 被忽略，`<style>` 元素的 `textContent` 赋值被阻止 → 改用 `CSSStyleSheet` 构造样式表（`adoptedStyleSheets` API），绕过 CSP inline 限制
  3. 缺少诊断日志无法定位故障点 → 全链路添加日志（emit 成功/失败、listener 注册、CSS 注入、payload 校验）
- **CSP 阻止 IPC 通信**：Tauri 2 的 IPC 通过 `http://ipc.localhost/` 发送请求，但 CSP `connect-src` 仅配置了 `ipc:` 而未包含 `http://ipc.localhost`，导致所有 IPC 调用（`playlist_get_play_mode`、`skin_list`、`plugin:event|listen` 等）被 CSP 拦截并回退到 `postMessage` 接口。已在 CSP 添加 `http://ipc.localhost`
- **Windows 构建脚本间歇性 panic**：`embed_resource` crate 在调用 `rustc_version::version_meta()` 时触发 Rust std 的 `Command::output` Windows 已知 bug（返回 `code: 0` 但被当作 `Err`），导致 release/debug 构建随机失败。已在 `build.rs` 添加 `catch_unwind` + 5 次重试机制，重试均失败时回退到最小 cfg 保证可编译
- **切歌 metadata 错位**：切歌瞬间前端使用 `fileChanged` 守卫保留旧 metadata，但当新歌异步标签读取在新歌首 tick 前完成时，`file_changed` 与 `metadata_changed` 同时为真会导致后端只发送一次新 metadata，前端守卫却永久丢弃它。简化为 `p.metadata ?? cur.metadata`，依赖后端 `is_current` 校验保证 payload 始终对应当前文件
- **桌面歌词横屏左侧文字截断**：`canvas.measureText` 与 WebView 实际文本渲染宽度存在子像素差异（生产环境字体 hinting/抗锯齿可能让实际宽度略大于测量值），单行模式下 `padH=20` 不足，添加按字号比例缩放的安全余量（`max(8, fontSize * 0.25)`）防止左侧文字被截断

#### 改进

- **桌面歌词 CSS 变量 fallback**：`lyrics-desktop.html` 添加与 `default` 皮肤对齐的 CSS 变量默认值，确保从窗口创建到皮肤 CSS 异步注入完成期间也有正确的视觉表现，避免边框透明、颜色不应用主题的闪烁
- **皮肤同步诊断日志**：主窗口 `applySkin`、`SettingsPanel` 主题切换、桌面歌词 `useSkinCss` 全链路添加结构化日志（`[TTPlayer]` 前缀），便于排查事件送达、监听器注册、CSS 注入各环节故障
- **入口属性位置修正**：将 `windows_subsystem = "windows"` 属性从 `lib.rs` 移至 `main.rs`，符合 Tauri 2 二进制入口约定

#### 重大变更

无

---

### v0.2.0

本次发布聚焦于桌面歌词、迷你模式与播放稳定性的深度优化，涵盖 36 个文件、约 2600 行变更。

#### 新增功能

- **桌面歌词状态记忆**：桌面歌词窗口的开启/关闭状态持久化到本地配置文件，应用重启后自动恢复上次状态
- **桌面歌词横竖屏分别记忆**：横屏与竖屏模式的窗口位置/尺寸分别保存到 `localStorage`，互不覆盖，使用物理坐标（`PhysicalPosition`/`PhysicalSize`）确保多 DPI 环境零误差恢复
- **桌面歌词皮肤主题集成**：文本颜色（正常/高亮/非当前行）、背景、边框、进度条（已播/未播）、控制栏、角落按钮交互态等全面跟随系统皮肤与主题，亮/暗模式实时同步
- **标题超长滚动显示**：主界面歌曲标题超出容器宽度时自动激活平滑横向滚动（canvas `measureText` 测量 + `ResizeObserver` 响应容器变化），速度恒定 ~50px/s，鼠标悬停暂停
- **菜单跟随皮肤主题**：「添加文件」下拉菜单与播放列表播放模式菜单改用 CSS 变量（`--bg-tertiary`、`--border-color`、`--accent-rgb` 等），与系统皮肤/主题保持一致
- **标签即时刷新**：手动编辑并保存歌曲标签后，播放器界面立即刷新元数据显示，无需手动刷新或重启，且不干扰播放状态
- **文件名回退显示**：无标签的音频文件回退显示文件名，并支持滚动动画

#### 修复的问题

- **随机模式标签错位**：快速切歌时旧歌的异步标签读取会覆盖新歌的 `metadata`，现通过后端 `current_file` 校验（`is_current` 检查丢弃非当前文件的标签）保障 `player.metadata()` 始终对应当前文件；前端直接采用 payload 中的 metadata（曾用 `fileChanged` 守卫保留旧值，但当新歌标签读取在新歌首 tick 前完成时会永久丢弃新 metadata，已在 0.2.x 修复）
- **进度条点击回退**：点击进度条跳转后短暂回退到原位置再跳转，根因为后端 50ms 事件推送的旧位置数据覆盖乐观更新，现通过 `seekingTo` 守卫过滤陈旧位置更新
- **迷你模式导致桌面歌词冻结**：切换到迷你模式后桌面歌词窗口无响应且歌词不更新，根因为 `useDesktopLyrics` hook 随 `LyricsPanel` 卸载而 cleanup，已将 hook 提升至 `MainPanel`（`if (miniMode) return` 之前）保持活跃
- **迷你模式切换闪现**：进入/退出迷你模式时短暂闪现旧界面帧，通过在 `hide()` 前设置 `opacity=0` 并等待两次 `requestAnimationFrame` 确保 OS 缓存空白帧
- **迷你模式按钮与拖拽失效**：`setResizable(false)` 在 Tauri 2 中破坏 `data-tauri-drag-region` 并导致布局异常，改用 `minSize == maxSize` 保持窗口不可缩放同时维持可拖拽；`data-tauri-drag-region` 改用 `"deep"` 模式支持子树拖拽；React 状态更新改用 `flushSync` 确保 DOM 提交后再调整窗口尺寸
- **桌面歌词窗口偶发无响应**：锁定轮询产生大量无变化 IPC 调用（50 次/秒）+ 无节流 `lyrics-update`（20Hz）+ 60Hz RAF 空转导致 IPC 拥塞与渲染积压，已通过缓存上次值仅在变化时调用 IPC、`lyrics-update` 节流到 ~8Hz、RAF 收敛后停止循环三项优化解决
- **窗口位置/大小监听器泄漏**：`onMoved`/`onResized` 的 `unlisten` 局部变量模式在 Promise resolve 前卸载时命中 `undefined`，改为 `promise.then(fn => fn())` 模式确保竞态安全
- **切歌后桌面歌词残留**：切换到无歌词歌曲时桌面歌词仍显示上一首内容，现发送空 payload 清空并显示「♪ 暂无歌词 ♪」
- **切歌后歌词面板不重置**：主窗口歌词面板在切歌后未回到顶部，通过 `linesContainerRef` 监听 `currentFile` 变化时重置 `scrollTop = 0`
- **Crossfade panic**：解码线程（非 Tokio 线程）中调用 `tokio::task::spawn_blocking` 导致 "no reactor running" panic，改用 `rt_handle.spawn_blocking` 通过 `Handle` 提交阻塞任务到 Tokio 运行时
- **ID3v2 标签读取失败**：部分文件的 ID3v2 时间戳帧（`TDRC`/`TYER`）含非 ASCII 字符，在默认 `BestAttempt` 模式下导致整个标签读取失败，现使用 `ParsingMode::Relaxed` 丢弃无效字段继续解析
- **应用关闭时桌面歌词状态错误持久化**：`tauri://destroyed` 事件无法区分用户关闭窗口与应用关闭，导致应用关闭时 `visible` 被错误设为 `false`，通过 `isAppClosingRef` 标志区分

#### 性能改进

- **桌面歌词锁定轮询优化**：缓存窗口几何（`outerPosition`/`outerSize`/`scaleFactor`），仅在首次或失效时查询，减少 IPC 调用；缓存 `setIgnoreCursorEvents`/`setCornerHovered` 上次值，仅在变化时调用
- **桌面歌词事件节流**：`lyrics-update` 的 `currentIndex` 变化时强制推送，`progress` 变化时节流到 120ms（~8Hz），避免 20Hz 无节流 `emitTo` 导致桌面窗口 IPC 反序列化与 React 渲染积压
- **桌面歌词 RAF 优化**：逐字动画的 `requestAnimationFrame` 在收敛（`|diff| < 0.03`）后停止循环，`target` 变化时重启，降低暂停/无歌词时的空闲 CPU 占用
- **窗口几何物理坐标恢复**：桌面歌词位置/尺寸使用 `PhysicalPosition`/`PhysicalSize`，避免逻辑坐标在多 DPI 环境下的精度损失

#### 其他变更

- 播放状态文本 `Stereo`/`Mono` 改为中文「立体声」/「单声道」
- 主界面歌曲标题最大显示宽度从 200px 缩短至 140px，为进度条与控制按钮留出更多空间，艺术家名同步对齐
- 应用图标资源更新
- 迷你模式窗口位置持久化到 `localStorage`（key: `ttplayer:mini-mode-pos`），重启后恢复

---

## 许可证

本项目基于 [MIT License](LICENSE) 开源。

## 联系方式

- 仓库地址：[https://github.com/ttplayer-next/ttplayer-next](https://github.com/ttplayer-next/ttplayer-next)
- Issue 反馈：[https://github.com/ttplayer-next/ttplayer-next/issues](https://github.com/ttplayer-next/ttplayer-next/issues)

---

<p align="center">
  如果本项目对您有帮助，欢迎 Star ⭐ 支持！
</p>
