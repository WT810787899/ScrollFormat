# 格式卷轴 ScrollFormat

> 一卷纳万格，转换自天成
> 本地优先的多格式转换工具 · Tauri 2 + Rust + React 19

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

所有转换均在本机完成，**文件不上传、不外发**。

---

## 特性

| 模块 | 能力 |
|---|---|
| 工作台 | 图片 / 文档 / 音频 / 视频 四个页签，拖拽或点选导入，格式自动识别 |
| 图片转换 | jpg · png · webp · gif · bmp · tiff · ico；质量、长边限制、按比例缩放、插值算法、透明区域填充 |
| 音频转换 | mp3 · wav · flac · aac · ogg · m4a · opus；码率 / 采样率 / 声道 / 编码器 / 音量归一化 |
| 视频转换 | mp4 · mkv · mov · webm · avi · gif；CRF · 分辨率 · 帧率 · 去隔行 · 编码器（含硬件加速）· 音频轨参数 |
| 任务队列 | 每个文件一张任务卡片，实时进度、取消、暂停/继续、重试、删除、批量操作、拖拽框选 |
| 转码日志 | 实时推送 ffmpeg 输出，底部单行日志栏，卡片内可展开完整日志 |
| 命名规则 | 前缀 / 后缀 / 序号 / 时间戳；冲突策略：自动重命名 / 覆盖 / 跳过 |
| 输出目录 | 项目默认 / 原文件地址 / 自定义，按类型记忆 |
| 预设 | 内置四种 + 自定义预设，保存与读取整套参数区 |
| 环境检测 | CPU、内存、GPU（nvidia-smi）、临时目录、外部工具可用性与来源 |
| 界面 | 毛玻璃 UI、关键色机制（21 色 + 自定义取色）、壁纸与磨砂可调、深/浅色主题、无边框窗口 |

---

## 快速开始

环境要求：Rust 1.80+、Node 18+（20 LTS 推荐）、Windows WebView2 / macOS / Linux 桌面环境。

```bash
# 安装前端依赖
cd ui && npm install && cd ..

# 开发模式（Vite 热更新 + Rust 自动重编译）
npx --prefix ui tauri dev

# 打包
npx --prefix ui tauri build
```

> `tauri dev` 需在**项目根目录**执行。

### CLI

```bash
cargo run -p scrollfmt -- convert input.png -o output.webp   # 单文件转换
cargo run -p scrollfmt -- env                                  # 环境检测
cargo run -p scrollfmt -- task-list                             # 历史任务
```

---

## 项目结构

```
scroll-format/
├── crates/
│   ├── core/          领域核心：Task / Format / Converter trait / 命名引擎 / 错误码
│   ├── infra/         基础设施：ProcessRunner / SQLite / 环境与 GPU 探测 / 配置存储
│   ├── converters/    转换器：图片 / 音频 / 视频 / 占位转换器
│   ├── app/           应用服务：任务队列、调度、暂停继续、事件
│   └── cli/           scrollfmt 命令行
├── src-tauri/         Tauri 命令、事件桥、窗口配置
├── ui/                React 前端（components / features / lib / store）
├── sidecars/          可选：随包分发的 ffmpeg 等
├── presets/           可选：预设模板
├── docs/              架构与说明文档
└── .scrollformat/     运行时数据（自动创建）
```

---

## 使用说明

1. 顶部选择类型页签，拖入或点击选择文件
2. 右侧配置：输出格式、预设、质量/CRF、输出目录、命名规则、高级参数
3. 点击顶栏「开始转换」（或空白处右键菜单）
4. 切换「任务列表」查看进度与日志，可暂停 / 继续 / 重试 / 取消 / 删除
5. 顶栏「设置」可调整主题、关键色、壁纸、磨砂参数、外部工具路径与并发数

交互速查：

- 任务卡片：**单击**选中，**Ctrl + 单击**多选，**空白处按住拖拽**框选，**右键**更多操作
- 下拉菜单为自绘毛玻璃组件，支持 Esc 关闭与自动向上弹出

---

## 数据与配置

运行时数据全部保存在**项目本地** `.scrollformat/`（可用环境变量 `SCROLLFORMAT_DATA_DIR` 覆盖）：

| 文件 | 内容 |
|---|---|
| `scrollformat.db` | SQLite：任务、任务项、状态与参数快照 |
| `settings.json` | 主题、并发、日志级别、超时、外部工具路径、窗口尺寸 |
| `presets/*.json` | 用户自定义预设 |
| `ui_state.json` | 界面状态：关键色、壁纸、磨砂参数、分割比例、各类型输出目录 |

异常退出时进行中的任务会被标记为中断，重启后可重试。

---

## 外部工具

探测顺序：**设置中手动指定 → sidecar → 系统 PATH → 常见安装目录**。

在「设置 → 转换」可为 `ffmpeg`、`ffprobe` 指定绝对路径；「环境检测」页显示各工具来源与版本。

- 音视频转换依赖 `ffmpeg` / `ffprobe`
- 图片转换不依赖外部工具（Rust `image` 库）
- 文档 / 电子书 / 数据 / 压缩包 / 字体：转换器接口已注册，界面暂显示占位提示

---

## 安全约束

- 外部进程一律 `Command::new` + 参数数组，**无 shell 字符串拼接**
- 打开外链仅允许 `http/https`
- 输出默认自动重命名，不覆盖原文件；命名结果过滤非法字符
- 统一错误码（`FMT-*`）与中文提示
- 纯本地运行，无任何数据上传行为

---

## 许可

[MIT License](LICENSE) © 2026 格式卷轴 ScrollFormat
第三方组件与外部工具许可见 [NOTICE](NOTICE)；架构细节见 [docs/架构说明.md](docs/架构说明.md)。