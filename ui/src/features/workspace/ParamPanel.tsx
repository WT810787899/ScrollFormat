// ═══════════════════════════════════════════════════════════
// ParamPanel —— 参数区 UI（工作台 TAB 与任务卡片「修改参数」弹窗共用同一份模板）
//   受控用法：value = ParamBag，onChange(patch) 局部更新
// ═══════════════════════════════════════════════════════════
import React from "react";
import { FolderOpen } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { GlassSelect } from "../../components/GlassSelect";
import { GlassCheckbox } from "../../components/GlassCheckbox";
import { ParamField } from "../../components/ParamField";
import { Card } from "../../components/Card";
import { Kind, KIND_META } from "../../lib/kinds";
import { Preset } from "../../lib/api";
import { ParamBag } from "./paramModel";

export function ParamPanel({
  kind,
  value,
  onChange,
  errors = {},
  presets,
  onPreset,
  presetId,
  onSavePreset,
  onDeletePreset,
  defaultOutDirText,
}: {
  kind: Kind;
  value: ParamBag;
  onChange: (patch: ParamBag) => void;
  errors?: Record<string, string>;
  /** 传入则显示预设行（弹窗里可只读展示，不传则隐藏） */
  presets?: Preset[];
  presetId?: string;
  onPreset?: (id: string) => void;
  onSavePreset?: () => void;
  onDeletePreset?: () => void;
  defaultOutDirText?: string;
}) {
  const meta = KIND_META[kind];
  const set = <K extends keyof ParamBag>(k: K, v: ParamBag[K]) => onChange({ [k]: v } as ParamBag);
  const setNaming = (patch: Partial<ParamBag["naming"]>) => onChange({ naming: { ...value.naming, ...patch } });
  const naming = value.naming;

  return (
    // flex-none：作为滚动容器的子项时不被压缩（否则卡片会被挤扁而不是正常滚动）
    <div className="flex-none flex flex-col gap-2">
      {presets && (
        <Card className="py-2">
          <div className="flex items-center gap-2">
            <span className="text-xs t-3 w-16 shrink-0 pl-1">预设</span>
            <GlassSelect value={presetId ?? ""} onChange={(v: string) => onPreset?.(v)}>
              <option value="">— 不使用 —</option>
              {presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.builtin ? "★ " : ""}
                  {p.name}
                </option>
              ))}
            </GlassSelect>
            {onSavePreset && (
              <button onClick={onSavePreset} title="把当前参数区全部保存为预设" className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25 whitespace-nowrap">
                存为预设
              </button>
            )}
            {onDeletePreset && presetId && (
              <button onClick={onDeletePreset} title="删除当前预设" className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-rose-500/25 whitespace-nowrap">
                删除
              </button>
            )}
          </div>
        </Card>
      )}

      <Card title="基础参数">
        <div className="flex flex-col gap-2">
          <ParamField label="输出格式" error={errors.target}>
            <GlassSelect value={value.target} onChange={(v: string) => set("target", v)}>
              {meta.outputs.map((o) => (
                <option key={o} value={o}>
                  {o}
                </option>
              ))}
            </GlassSelect>
          </ParamField>
          <ParamField label="质量" error={errors.quality} wide>
            <div className="flex items-center gap-2">
              <input type="range" min={10} max={100} value={value.quality} onChange={(e) => set("quality", +e.target.value)} className="flex-1" />
              <span className="text-xs opacity-70 w-8">{value.quality}</span>
            </div>
          </ParamField>
          <ParamField label="输出目录" error={errors.outDir} wide>
            <div className="flex items-center gap-1.5">
              {(
                [
                  { v: "project_default", label: "应用默认" },
                  { v: "source_dir", label: "原文件地址" },
                  { v: "custom", label: "自定义" },
                ] as const
              ).map((m) => (
                <button
                  key={m.v}
                  onClick={() => set("outDirMode", m.v)}
                  title={`输出到${m.label}`}
                  className={`h-7 px-2.5 text-xs rounded-lg whitespace-nowrap transition ${
                    value.outDirMode === m.v ? "tab-active" : "bg-slate-500/15 hover:bg-slate-500/25"
                  }`}
                >
                  {m.label}
                </button>
              ))}
              {value.outDirMode === "custom" && (
                <button
                  onClick={async () => {
                    const r = await openDialog({ directory: true });
                    if (typeof r === "string") set("outDir", r);
                  }}
                  title="选择输出目录"
                  className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25 flex items-center gap-1.5 min-w-0"
                >
                  <FolderOpen className="w-3.5 h-3.5 flex-none" />
                  <span className="truncate max-w-[220px]" title={value.outDir}>
                    {value.outDir ? value.outDir.split(/[\\/]/).pop() : "选择目录"}
                  </span>
                </button>
              )}
              {value.outDirMode !== "custom" && (
                <span className="text-[11px] t-3 truncate">
                  {value.outDirMode === "source_dir" ? "输出到每个源文件所在目录" : `项目默认：${defaultOutDirText || "未设置（请到设置中指定）"}`}
                </span>
              )}
            </div>
          </ParamField>
          <ParamField label="命名选项" error={errors.naming} wide>
            <div className="flex items-center gap-2">
              <div className="flex items-center gap-1">
                <span className="text-xs t-3">前缀</span>
                <input
                  value={naming.prefix}
                  onChange={(e) => setNaming({ prefix: e.target.value })}
                  placeholder="自定义"
                  className="h-7 w-20 bg-slate-500/15 hover:bg-slate-500/25 rounded-lg px-2 text-xs"
                />
              </div>
              <div className="flex items-center gap-1">
                <span className="text-xs t-3">后缀</span>
                <input
                  value={naming.suffix}
                  onChange={(e) => setNaming({ suffix: e.target.value })}
                  placeholder="自定义"
                  className="h-7 w-20 bg-slate-500/15 hover:bg-slate-500/25 rounded-lg px-2 text-xs"
                />
              </div>
              <GlassCheckbox checked={naming.useIndex} onChange={(v) => setNaming({ useIndex: v })} label="序号" title="在文件名中加入 001 形式的序号" />
              <GlassCheckbox checked={naming.useTimestamp} onChange={(v) => setNaming({ useTimestamp: v })} label="时间" title="在文件名中加入日期（yyyyMMdd）" />
            </div>
          </ParamField>
          <ParamField label="冲突策略">
            <GlassSelect value={naming.conflict} onChange={(v: string) => setNaming({ conflict: v })}>
              <option value="auto_rename">自动重命名</option>
              <option value="overwrite">覆盖</option>
              <option value="skip">跳过</option>
            </GlassSelect>
          </ParamField>
        </div>
      </Card>

      <Card title="高级参数">
        <div className="flex flex-col gap-2">
          {kind === "image" && (
            <>
              <ParamField label="缩放方式" wide>
                <div className="flex items-center gap-2">
                  <GlassSelect value={value.imgScaleMode} onChange={(v: string) => set("imgScaleMode", v)}>
                    <option value="long">长边限制</option>
                    <option value="percent">按比例缩放</option>
                    <option value="none">不缩放</option>
                  </GlassSelect>
                  {value.imgScaleMode === "long" && (
                    <GlassSelect value={value.maxSide} onChange={(v: string) => set("maxSide", v)}>
                      {["original", "4096", "3840", "2560", "2048", "1920", "1600", "1440", "1280", "1080", "1024", "800", "640", "512", "320"].map((o) => (
                        <option key={o} value={o}>
                          {o === "original" ? "原始" : o}
                        </option>
                      ))}
                    </GlassSelect>
                  )}
                  {value.imgScaleMode === "percent" && (
                    <div className="flex items-center gap-2 flex-1">
                      <input type="range" min={5} max={200} value={value.imgPercent} onChange={(e) => set("imgPercent", +e.target.value)} className="flex-1" />
                      <span className="text-xs t-3 w-10 text-right tabular-nums">{value.imgPercent}%</span>
                    </div>
                  )}
                </div>
              </ParamField>
              <ParamField label="插值算法" hint="缩小时的采样质量">
                <GlassSelect value={value.imgFilter} onChange={(v: string) => set("imgFilter", v)}>
                  <option value="lanczos3">Lanczos3（默认）</option>
                  <option value="catmullrom">CatmullRom</option>
                  <option value="triangle">Triangle（双线性）</option>
                  <option value="nearest">Nearest（最近邻）</option>
                </GlassSelect>
              </ParamField>
              <ParamField label="输出位深">
                <GlassSelect value={value.imgDepth} onChange={(v: string) => set("imgDepth", v)}>
                  <option value="auto">自动</option>
                  <option value="8">8 bit</option>
                  <option value="16">16 bit</option>
                </GlassSelect>
              </ParamField>
              <ParamField label="背景色" hint="JPEG 等不支持透明通道时填充的颜色" wide>
                <div className="flex items-center gap-2">
                  <input type="color" value={value.imgBg} onChange={(e) => set("imgBg", e.target.value)} className="w-8 h-7 rounded-lg cursor-pointer" />
                  <GlassCheckbox checked={value.imgFlatten} onChange={(v) => set("imgFlatten", v)} label="透明区域填充" title="PNG/WebP 转 JPEG 时用背景色填充透明区域" />
                </div>
              </ParamField>
            </>
          )}
          {kind === "audio" && (
            <>
              <ParamField label="采样率">
                <GlassSelect value={value.sampleRate} onChange={(v: string) => set("sampleRate", v)}>
                  <option value="original">原始</option>
                  <option value="22050">22050</option>
                  <option value="32000">32000</option>
                  <option value="44100">44100</option>
                  <option value="48000">48000</option>
                  <option value="96000">96000</option>
                </GlassSelect>
              </ParamField>
              <ParamField label="声道">
                <GlassSelect value={value.channels} onChange={(v: string) => set("channels", v)}>
                  <option value="1">单声道</option>
                  <option value="2">立体声</option>
                  <option value="6">5.1 环绕</option>
                </GlassSelect>
              </ParamField>
              <ParamField label="编码器/码率" hint="编码器留空按格式自动" wide>
                <div className="flex items-center gap-2">
                  <GlassSelect value={value.audioCodec} onChange={(v: string) => set("audioCodec", v)}>
                    <option value="">自动</option>
                    <option value="libmp3lame">MP3 (libmp3lame)</option>
                    <option value="libvorbis">Vorbis (libvorbis)</option>
                    <option value="libopus">Opus (libopus)</option>
                    <option value="aac">AAC</option>
                    <option value="flac">FLAC</option>
                    <option value="pcm_s16le">PCM 16bit</option>
                    <option value="wmav2">WMA</option>
                  </GlassSelect>
                  <GlassSelect value={value.bitrate} onChange={(v: string) => set("bitrate", v)}>
                    {["64k", "96k", "128k", "160k", "192k", "256k", "320k", "lossless"].map((o) => (
                      <option key={o} value={o}>
                        {o}
                      </option>
                    ))}
                  </GlassSelect>
                </div>
              </ParamField>
              <ParamField label="音量归一化" hint="loudnorm 双声道响度标准化">
                <div className="flex items-center gap-2">
                  <GlassSelect value={value.loudnorm} onChange={(v: string) => set("loudnorm", v)}>
                    <option value="off">关闭</option>
                    <option value="ebur128">EBU R128 (-14 LUFS)</option>
                    <option value="ebur128_16">EBU R128 峰值 -16</option>
                    <option value="dynaudnorm">动态模式</option>
                  </GlassSelect>
                </div>
              </ParamField>
              <ParamField label="保留元数据">
                <GlassCheckbox checked={value.keepMeta} onChange={(v) => set("keepMeta", v)} label="复制封面与标签" />
              </ParamField>
            </>
          )}
          {kind === "video" && (
            <>
              <ParamField label="CRF" error={errors.crf} hint="数值越小画质越好、体积越大" wide>
                <div className="flex items-center gap-2">
                  <input type="range" min={0} max={51} value={value.crf} onChange={(e) => set("crf", +e.target.value)} className="flex-1" />
                  <span className="text-xs t-3 w-8 text-right tabular-nums">{value.crf}</span>
                </div>
              </ParamField>
              <ParamField label="分辨率">
                <div className="flex items-center gap-2">
                  <GlassSelect value={value.vscale} onChange={(v: string) => set("vscale", v)}>
                    <option value="original">原始</option>
                    <option value="3840:-2">4K 2160p</option>
                    <option value="2560:-2">2K 1440p</option>
                    <option value="1920:-2">1080p</option>
                    <option value="1600:-2">1200p</option>
                    <option value="1280:-2">720p</option>
                    <option value="960:-2">540p</option>
                    <option value="854:-2">480p</option>
                    <option value="640:-2">360p</option>
                    <option value="480:-2">240p</option>
                    <option value="iw*3/4">宽度 75%</option>
                    <option value="iw/2">宽度 50%</option>
                    <option value="custom">自定义宽度</option>
                  </GlassSelect>
                  {value.vscale === "custom" && (
                    <input type="number" min={16} value={value.vcustomW} onChange={(e) => set("vcustomW", +e.target.value)} className="h-7 w-24 bg-slate-500/15 rounded-lg px-2 text-xs" />
                  )}
                </div>
              </ParamField>
              <ParamField label="帧率">
                <GlassSelect value={value.vfps} onChange={(v: string) => set("vfps", v)}>
                  {["original", "12", "15", "20", "23.976", "24", "25", "29.97", "30", "48", "50", "59.94", "60", "120"].map((o) => (
                    <option key={o} value={o}>
                      {o === "original" ? "原始" : o}
                    </option>
                  ))}
                </GlassSelect>
              </ParamField>
              <ParamField label="编码器/速度" wide>
                <div className="flex items-center gap-2">
                  <GlassSelect value={value.vcodec} onChange={(v: string) => set("vcodec", v)} title="硬件编码需显卡与驱动支持；速度取值随编码器自动适配">
                    <option value="auto_hw">硬件加速优先（推荐 · 有独显用 NVENC）</option>
                    <option value="">自动（按容器，CPU 软件编码）</option>
                    <option value="libx264">H.264 (libx264)</option>
                    <option value="libx265">H.265 (libx265)</option>
                    <option value="libvpx-vp9">VP9 (libvpx-vp9)</option>
                    <option value="libvpx">VP8 (libvpx)</option>
                    <option value="mpeg4">MPEG-4</option>
                    <option value="h264_nvenc">H.264 NVENC（显卡）</option>
                    <option value="hevc_nvenc">H.265 NVENC（显卡）</option>
                    <option value="h264_qsv">H.264 QSV（核显）</option>
                    <option value="h264_amf">H.264 AMF（AMD）</option>
                    <option value="libaom-av1">AV1 (libaom-av1)</option>
                  </GlassSelect>
                  <GlassSelect value={value.encPreset} onChange={(v: string) => set("encPreset", v)} title="软件编码器用 x264 风格速度名，硬件编码器会自动换算成 p1~p7 / 数字">
                    {["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"].map((o) => (
                      <option key={o} value={o}>
                        {o}
                      </option>
                    ))}
                  </GlassSelect>
                </div>
              </ParamField>
              <ParamField label="硬件解码" wide>
                <GlassCheckbox checked={value.hw_decode} onChange={(v) => set("hw_decode", v)} label="NVDEC / QSV 硬解" title="仅在选择硬件编码器时生效；GPU 不支持该源格式时自动改为 CPU 解码" />
              </ParamField>
              <ParamField label="去隔行">
                <GlassCheckbox checked={value.deinterlace} onChange={(v) => set("deinterlace", v)} label="启用 yadif 去隔行" title="使用 ffmpeg yadif 滤镜去除交错扫描的抖动" />
              </ParamField>
              <ParamField label="Web 优化">
                <GlassCheckbox checked={value.faststart} onChange={(v) => set("faststart", v)} label="moov 前置（+faststart）" title="MP4 索引前置，网页可边下边播" />
              </ParamField>
              <div className="h-px bg-white/10 my-0.5" />
              <ParamField label="音频编码" wide>
                <div className="flex items-center gap-2">
                  <GlassSelect value={value.acodec} onChange={(v: string) => set("acodec", v)}>
                    <option value="">自动</option>
                    <option value="aac">AAC</option>
                    <option value="libmp3lame">MP3</option>
                    <option value="libopus">Opus</option>
                    <option value="libvorbis">Vorbis</option>
                    <option value="flac">FLAC</option>
                    <option value="copy">直接复制（不重编码）</option>
                    <option value="none">去掉音轨</option>
                  </GlassSelect>
                  <GlassSelect value={value.abitrate} onChange={(v: string) => set("abitrate", v)}>
                    <option value="">默认码率</option>
                    {["64k", "96k", "128k", "192k", "256k", "320k"].map((o) => (
                      <option key={o} value={o}>
                        {o}
                      </option>
                    ))}
                  </GlassSelect>
                </div>
              </ParamField>
              <ParamField label="采样/声道" wide>
                <div className="flex items-center gap-2">
                  <GlassSelect value={value.asamplerate} onChange={(v: string) => set("asamplerate", v)}>
                    <option value="original">原始采样率</option>
                    <option value="22050">22050</option>
                    <option value="32000">32000</option>
                    <option value="44100">44100</option>
                    <option value="48000">48000</option>
                    <option value="96000">96000</option>
                  </GlassSelect>
                  <GlassSelect value={value.achannels} onChange={(v: string) => set("achannels", v)}>
                    <option value="">原始声道</option>
                    <option value="1">单声道</option>
                    <option value="2">立体声</option>
                    <option value="6">5.1 环绕</option>
                    <option value="8">7.1 环绕</option>
                  </GlassSelect>
                </div>
              </ParamField>
            </>
          )}
          {kind === "document" && (
            <>
              <ParamField label="导出页码" hint="仅在输出 png / jpg 时生效">
                <div className="flex items-center gap-2">
                  <GlassSelect
                    value={
                      value.doc_pages === "all" || !value.doc_pages ? "all"
                      : value.doc_pages === "first" ? "first"
                      : value.doc_pages === "1" ? "first"
                      : "custom"
                    }
                    onChange={(v: string) =>
                      set("doc_pages", v === "custom" ? (value.doc_pages && value.doc_pages !== "all" && value.doc_pages !== "first" ? value.doc_pages : "1-3") : v)
                    }
                  >
                    <option value="all">全部页</option>
                    <option value="first">只要首页</option>
                    <option value="custom">自定义…</option>
                  </GlassSelect>
                  {value.doc_pages !== "all" && value.doc_pages !== "first" && (
                    <input
                      value={value.doc_pages}
                      onChange={(e) => set("doc_pages", e.target.value)}
                      placeholder="1-3,5,8-"
                      className="h-7 w-[130px] bg-slate-500/15 rounded-lg px-2 text-xs font-mono"
                    />
                  )}
                </div>
              </ParamField>
              <ParamField label="页面分辨率" hint="仅在输出 png / jpg 时生效">
                <div className="flex items-center gap-2">
                  <GlassSelect value={String(value.doc_dpi)} onChange={(v: string) => set("doc_dpi", Number(v))}>
                    <option value="96">96 DPI（屏幕）</option>
                    <option value="150">150 DPI（推荐）</option>
                    <option value="300">300 DPI（打印）</option>
                    <option value="600">600 DPI（高清）</option>
                  </GlassSelect>
                </div>
              </ParamField>
              <ParamField label="图片格式" wide>
                <div className="flex items-center gap-2">
                  <GlassCheckbox
                    checked={value.doc_keep_bg}
                    onChange={(v) => set("doc_keep_bg", v)}
                    label="保留白底"
                    title="PNG 透明底；取消勾选输出 JPG"
                  />
                  <span className="text-[11px] t-3">PNG 输出透明背景，JPG 输出白底</span>
                </div>
              </ParamField>
              <div className="h-px bg-white/10 my-0.5" />
              <ParamField label="转换引擎" wide>
                <div className="flex items-center gap-2">
                  <span className="text-[11px] t-3 leading-relaxed">
                    txt / md / html / docx / odt / rtf / pdf 之间的文本转换、「转 PDF」以及
                    PDF 转 png/jpg 全部由内置引擎完成，无需安装任何软件；
                    中文会自动内嵌字体（文字可选中、可搜索）。
                    装了 LibreOffice 时优先用它以保留原始排版。
                  </span>
                </div>
              </ParamField>
            </>
          )}
          {(kind === "audio" || kind === "video") && (
            <>
              <div className="h-px bg-white/10 my-0.5" />
              <ParamField label="自定义参数" wide>
                <input
                  value={value.customArgs}
                  onChange={(e) => set("customArgs", e.target.value)}
                  placeholder={kind === "video" ? "-t 30 -profile:v high -movflags +faststart" : "-t 30 -map_metadata 0"}
                  className="h-7 w-full min-w-[240px] bg-slate-500/15 rounded-lg px-2 text-xs font-mono"
                />
              </ParamField>
            </>
          )}
        </div>
      </Card>
    </div>
  );
}
