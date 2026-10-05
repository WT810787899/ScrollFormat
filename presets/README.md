# presets（可选）

本目录用于存放**可分发的自定义预设模板**。

运行时用户预设保存在项目本地 `.scrollformat/presets/*.json`（不受本目录影响）。
如需随包附带示例预设，可将 JSON 文件放入本目录，安装时复制到 `.scrollformat/presets/` 即可。

## 预设结构

```json
{
  "id": "user_demo",
  "name": "示例：网页用 WebP 80",
  "builtin": false,
  "kind": "image",
  "options": {
    "target_ext": "webp",
    "quality": 80,
    "preset": null,
    "extra": {
      "scale_mode": "percent",
      "percent": 75,
      "filter": "lanczos3",
      "target_ext": "webp",
      "quality": 80,
      "naming": { "template": "{name}", "conflict": "auto_rename" },
      "out_dir_mode": "project_default"
    }
  },
  "out_dir_mode": "project_default",
  "out_dir": null,
  "naming": { "template": "{name}", "prefix": "", "suffix": "", "conflict": "auto_rename" },
  "updated_at": "2026-10-01T00:00:00Z"
}
```

字段说明：

| 字段 | 说明 |
|---|---|
| `id` / `name` | 预设标识与显示名 |
| `builtin` | 内置预设为 `true`（只读，UI 禁止删除/改名） |
| `kind` | 所属类型：`image` / `audio` / `video` / `document`，`null` 表示通用 |
| `options.extra` | **整个参数区**的快照，套用预设时据此完整回填 |
| `out_dir_mode` | `project_default` / `source_dir` / `custom` |
| `naming.template` | 命名模板，如 `{index:03}_{name}{timestamp}` |
