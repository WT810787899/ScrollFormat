import { Kind } from "./kinds";

export interface ValidationResult {
  ok: boolean;
  errors: Record<string, string>;
}

export function validateForm(input: {
  target: string;
  outDir: string;
  quality: number;
  crf: number;
  namingTemplate: string;
}): ValidationResult {
  const errors: Record<string, string> = {};
  if (!input.target) errors.target = "请选择输出格式";
  if (!input.outDir) errors.outDir = "请选择输出目录";
  if (input.quality < 10 || input.quality > 100) errors.quality = "质量范围 10–100";
  if (input.crf < 18 || input.crf > 35) errors.crf = "CRF 范围 18–35";
  if (input.namingTemplate && !/^[{}\w\-\.\u4e00-\u9fa5:0-9]+$/.test(input.namingTemplate)) {
    errors.naming = "命名规则含非法字符";
  }
  return { ok: Object.keys(errors).length === 0, errors };
}
