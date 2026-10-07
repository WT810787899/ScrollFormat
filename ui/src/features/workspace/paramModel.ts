// ═══════════════════════════════════════════════════════════
// 参数数据模型 —— 工作台 TAB 与任务卡片「修改参数」弹窗共用
//   工作台：defaultParams → 交互 → extraFromParams → 建任务
//   弹窗：  paramsFromExtra(任务已存参数) → 交互 → extraFromParams → update_task
// 两边模板完全一致，改一处两边同步生效。
// ═══════════════════════════════════════════════════════════
import { Kind, KIND_META } from "../../lib/kinds";
import { Preset } from "../../lib/api";

export interface NamingRule {
  prefix: string;
  suffix: string;
  useIndex: boolean;
  useTimestamp: boolean;
  conflict: "auto_rename" | "overwrite" | "skip";
}

/** 扁平参数表：一套参数 = 一个纯对象，便于整体存取与回填 */
export type ParamBag = Record<string, any>;

/** 目标是不是图片格式（决定页码范围等参数是否生效） */
export function isImageTarget(target: string): boolean {
  const t = (target || "").toLowerCase();
  return t === "png" || t === "jpg" || t === "jpeg" || t === "webp" || t === "bmp" || t === "tif" || t === "tiff";
}

/** 由前缀/后缀/序号/时间拼出后端模板 */
export function buildTemplate(n: NamingRule): string {
  return `${n.prefix}${n.useIndex ? "{index:03}" : ""}${n.useTimestamp ? "{timestamp}" : ""}{name}${n.suffix}`;
}

/** 从已存模板反解出界面状态 */
export function parseTemplate(rule: any): NamingRule {
  const tpl: string = rule?.template ?? "";
  const prefix: string = rule?.prefix ?? "";
  const suffix: string = rule?.suffix ?? "";
  const inner =
    tpl.startsWith(prefix) && tpl.endsWith(suffix) ? tpl.slice(prefix.length, tpl.length - suffix.length || undefined) : tpl;
  return {
    prefix,
    suffix,
    useIndex: inner.includes("{index"),
    useTimestamp: inner.includes("{timestamp"),
    conflict: rule?.conflict ?? "auto_rename",
  };
}

export const defaultNaming: NamingRule = {
  prefix: "",
  suffix: "",
  useIndex: false,
  useTimestamp: false,
  conflict: "auto_rename",
};

/** 各类参数的出厂默认值 */
export function defaultParams(kind: Kind): ParamBag {
  return {
    target: KIND_META[kind].outputs[0] || "",
    quality: 85,
    presetId: "",
    // 图片
    imgScaleMode: "long",
    maxSide: "original",
    imgPercent: 50,
    imgFilter: "lanczos3",
    imgDepth: "auto",
    imgBg: "#ffffff",
    imgFlatten: false,
    // 音频
    bitrate: "192k",
    sampleRate: "original",
    channels: "2",
    audioCodec: "",
    loudnorm: "off",
    keepMeta: true,
    // 视频
    crf: 23,
    vscale: "original",
    vcustomW: 1280,
    vfps: "original",
    encPreset: "veryfast",
    // 默认硬件加速优先：检测到独显用 NVENC，否则回落 CPU
    vcodec: "auto_hw",
    // 解码同样默认交给显卡（仅在硬件编码时生效）
    hw_decode: true,
    deinterlace: false,
    faststart: true,
    acodec: "",
    abitrate: "",
    asamplerate: "original",
    achannels: "",
    // 通用
    customArgs: "",
    naming: defaultNaming,
    outDirMode: "custom",
    outDir: "",
    // 文档
    doc_dpi: 150,
    doc_keep_bg: true,
    /** PDF 转图片的页码范围：all=全部页 / first=首页 / 其它=自定义表达式如 1-3,5 */
    doc_pages: "all",
  };
}

/** 从任务/预设存的 extra 回填成参数表（缺省项用默认值补齐） */
export function paramsFromExtra(kind: Kind, extra: any, base?: ParamBag): ParamBag {
  const p: ParamBag = { ...defaultParams(kind), ...(base || {}) };
  const e = extra || {};
  if (e.target_ext) p.target = e.target_ext;
  if (e.quality != null) p.quality = e.quality;
  if (e.out_dir_mode) p.outDirMode = e.out_dir_mode;
  if (e.out_dir != null) p.outDir = e.out_dir;
  if (e.naming) p.naming = parseTemplate(e.naming);
  if (e.custom_args != null) p.customArgs = e.custom_args;

  if (kind === "image") {
    if (e.scale_mode) p.imgScaleMode = e.scale_mode;
    else if (e.max_side != null) {
      p.imgScaleMode = "long";
      p.maxSide = String(e.max_side);
    }
    if (e.percent != null) p.imgPercent = e.percent;
    if (e.max_side != null && e.scale_mode !== "none") p.maxSide = String(e.max_side);
    if (e.filter) p.imgFilter = e.filter;
    if (e.depth) p.imgDepth = e.depth;
    if (e.flatten != null) p.imgFlatten = e.flatten;
    if (e.bg) p.imgBg = e.bg;
  }
  if (kind === "audio") {
    if (e.bitrate) p.bitrate = e.bitrate;
    if (e.sample_rate) p.sampleRate = e.sample_rate;
    if (e.channels != null) p.channels = String(e.channels);
    if (e.codec != null) p.audioCodec = e.codec;
    if (e.loudnorm) p.loudnorm = e.loudnorm;
    if (e.keep_meta != null) p.keepMeta = e.keep_meta;
  }
  if (kind === "document") {
    if (e.dpi != null) p.doc_dpi = e.dpi;
    if (e.keep_bg != null) p.doc_keep_bg = e.keep_bg;
    if (e.pages != null) p.doc_pages = String(e.pages);
  }
  if (kind === "video") {
    if (e.crf != null) p.crf = e.crf;
    if (e.scale_mode) p.vscale = e.scale_mode;
    else if (e.scale) p.vscale = e.scale;
    if (e.custom_width != null) p.vcustomW = e.custom_width;
    if (e.fps) p.vfps = e.fps;
    if (e.enc_preset) p.encPreset = e.enc_preset;
    if (e.v_codec != null) p.vcodec = e.v_codec;
    if (e.hw_decode != null) p.hw_decode = e.hw_decode;
    if (e.deinterlace != null) p.deinterlace = e.deinterlace;
    if (e.faststart != null) p.faststart = e.faststart;
    if (e.a_codec != null) p.acodec = e.a_codec;
    if (e.a_bitrate != null) p.abitrate = e.a_bitrate;
    if (e.a_sample_rate) p.asamplerate = e.a_sample_rate;
    p.achannels = e.a_channels ? String(e.a_channels) : "";
  }
  return p;
}

/** 参数表 → 建任务/改任务用的 extra（含格式、质量、目录、命名） */
export function extraFromParams(kind: Kind, p: ParamBag): any {
  let extra: any = {};
  if (kind === "image") {
    extra = {
      scale_mode: p.imgScaleMode,
      max_side: p.imgScaleMode === "long" && p.maxSide !== "original" ? Number(p.maxSide) : null,
      percent: p.imgScaleMode === "percent" ? p.imgPercent : null,
      filter: p.imgFilter,
      depth: p.imgDepth,
      flatten: p.imgFlatten,
      bg: p.imgBg,
    };
  }
  if (kind === "audio") {
    extra = {
      bitrate: p.bitrate,
      sample_rate: p.sampleRate,
      channels: Number(p.channels),
      codec: p.audioCodec,
      loudnorm: p.loudnorm,
      keep_meta: p.keepMeta,
      custom_args: p.customArgs,
    };
  }
  if (kind === "video") {
    extra = {
      crf: p.crf,
      scale_mode: p.vscale,
      custom_width: p.vcustomW,
      scale: p.vscale === "custom" ? `${p.vcustomW}:-2` : p.vscale,
      fps: p.vfps,
      enc_preset: p.encPreset,
      v_codec: p.vcodec,
      hw_decode: p.hw_decode,
      deinterlace: p.deinterlace,
      faststart: p.faststart,
      a_codec: p.acodec,
      a_bitrate: p.abitrate,
      a_sample_rate: p.asamplerate,
      a_channels: p.achannels ? Number(p.achannels) : 0,
      custom_args: p.customArgs,
    };
  }
  if (kind === "document") {
    extra = {
      dpi: p.doc_dpi,
      keep_bg: p.doc_keep_bg,
      // 只在目标是图片时才有意义（PDF/docx → png/jpg 拆页）
      pages: isImageTarget(p.target) ? p.doc_pages || "all" : "all",
    };
  }
  return {
    ...extra,
    target_ext: p.target,
    quality: p.quality,
    naming: { ...p.naming, template: buildTemplate(p.naming) },
    out_dir_mode: p.outDirMode,
    out_dir: p.outDir,
  };
}

/** 套用预设：等价于工作台里选择预设后的整套回填 */
export function paramsFromPreset(kind: Kind, preset: Preset, current: ParamBag): ParamBag {
  const extra: any = (preset.options as any)?.extra || {};
  const p = paramsFromExtra(kind, { ...extra, ...(extra.target_ext ? {} : {}) }, current);
  const targetExt = extra.target_ext || preset.options.target_ext;
  if (targetExt) p.target = targetExt;
  const q = extra.quality ?? preset.options.quality;
  if (q != null) p.quality = q;
  p.outDirMode = extra.out_dir_mode || preset.out_dir_mode;
  if (extra.out_dir || preset.out_dir) p.outDir = extra.out_dir || preset.out_dir;
  if (extra.naming) p.naming = parseTemplate(extra.naming);
  else if (preset.naming) p.naming = parseTemplate(preset.naming);
  p.presetId = preset.id;
  return p;
}
