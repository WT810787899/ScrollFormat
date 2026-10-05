// codemod v2：<select> -> <GlassSelect>（保持 options 为 JSX 子节点，支持动态 map）
const fs = require("fs");
const path = require("path");

function findBlocks(src) {
  const blocks = [];
  const re = /<select\b/g;
  let m;
  while ((m = re.exec(src))) {
    const start = m.index;
    // 找到配对的 </select>
    const endTag = src.indexOf("</select>", start);
    if (endTag === -1) continue;
    // 扫描开标签结束位置（跳过 {} 内的 > ）
    let i = start + "<select".length;
    let depth = 0;
    let tagEnd = -1;
    while (i < src.length) {
      const c = src[i];
      if (c === "{") depth++;
      else if (c === "}") depth--;
      else if (c === ">" && depth === 0) { tagEnd = i; break; }
      i++;
    }
    if (tagEnd === -1) continue;
    blocks.push({ start, openEnd: tagEnd + 1, end: endTag + "</select>".length });
    re.lastIndex = endTag;
  }
  return blocks;
}

function rewriteOnChange(openTag) {
  const key = "onChange=";
  const idx = openTag.indexOf(key);
  if (idx === -1) return { openTag, changed: false };
  let i = idx + key.length;
  if (openTag[i] !== "{") return { openTag, changed: false };
  let depth = 0;
  let j = i;
  for (; j < openTag.length; j++) {
    if (openTag[j] === "{") depth++;
    else if (openTag[j] === "}") {
      depth--;
      if (depth === 0) break;
    }
  }
  let expr = openTag.slice(i + 1, j); // (e) => ...
  const orig = expr;
  expr = expr.replace(/\(\s*e\s*\)\s*=>/, "(v: string) =>");
  expr = expr.replace(/e\.target\.value/g, "v");
  if (expr === orig) return { openTag, changed: false };
  return { openTag: openTag.slice(0, i + 1) + expr + openTag.slice(j), changed: true };
}

const files = ["src/features/settings/SettingsTab.tsx", "src/features/workspace/KindTab.tsx"];

for (const rel of files) {
  const file = path.join(__dirname, "..", rel);
  let src = fs.readFileSync(file, "utf8");
  let n = 0;
  const blocks = findBlocks(src);
  for (let k = blocks.length - 1; k >= 0; k--) {
    const b = blocks[k];
    const openTag = src.slice(b.start, b.openEnd);
    const r = rewriteOnChange(openTag);
    let newOpen = r.openTag.replace("<select", "<GlassSelect");
    // 宽度类交给 GlassSelect 默认 w-full，保留其余
    newOpen = newOpen.replace(/\sclassName="(w-full|flex-1)"/g, "");
    const newOpenTag = newOpen;
    const body = src.slice(b.openEnd, b.end - "</select>".length);
    const newClose = "</GlassSelect>";
    src = src.slice(0, b.start) + newOpenTag + body + newClose + src.slice(b.end);
    n++;
  }
  if (n && !/import \{ GlassSelect \}/.test(src)) {
    src = src.replace(/^(import .*\n)/m, '$1import { GlassSelect } from "../../components/GlassSelect";\n');
  }
  fs.writeFileSync(file, src, "utf8");
  console.log(rel, "->", n, "blocks");
}
