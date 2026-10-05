# sidecars（可选）

本目录用于存放**随包分发**的外部工具，ScrollFormat 的工具探测顺序为：

1. 设置中手动指定的路径（`settings.json` 的 `ffmpeg_path` / `ffprobe_path`）
2. sidecar：程序同级目录，或本 `sidecars/` 目录
3. 系统 `PATH`
4. 常见安装目录（`C:\Program Files\ffmpeg\bin`、`/usr/local/bin` 等）

## 建议放置的文件

```
sidecars/
├── ffmpeg.exe        # Windows
├── ffprobe.exe
├── ffmpeg            # macOS / Linux
├── ffprobe
├── pandoc(.exe)      # 后续：文档转换
├── soffice(.exe)     # 后续：办公文档转换（LibreOffice）
├── ebook-convert(.exe) # 后续：电子书转换（calibre）
└── 7z.exe            # 后续：压缩包处理
```

## 注意事项

- 二进制体积较大（ffmpeg 约 80–120 MB），仓库默认**不提交**，需在发布流程中拷贝进来
- 分发前请核对对应许可：FFmpeg 为 LGPL-2.1+ 或 GPL（取决于构建），Pandoc 为 GPL-2.0+，LibreOffice 为 MPL-2.0，7-Zip 为 LGPL-2.1+
- Windows 建议使用 `x86_64` 构建并命名为 `ffmpeg.exe` / `ffprobe.exe`
