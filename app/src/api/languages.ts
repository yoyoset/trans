/**
 * 界面上可选的语言。翻译引擎只认标签（`ja`、`ko`…），提示词里原样带过去，所以新增语言只要加一行。
 * OCR 能识别哪些语言取决于系统装了什么（见 `listOcrLanguages`），和这里是两回事。
 */
export const LANGUAGES: Array<{ tag: string; label: string }> = [
  { tag: "zh", label: "中文" },
  { tag: "en", label: "英语" },
  { tag: "ja", label: "日语" },
  { tag: "ko", label: "韩语" },
  { tag: "es", label: "西班牙语" },
  { tag: "fr", label: "法语" },
  { tag: "de", label: "德语" },
  { tag: "ru", label: "俄语" },
  { tag: "pt", label: "葡萄牙语" },
  { tag: "it", label: "意大利语" },
  { tag: "vi", label: "越南语" },
  { tag: "th", label: "泰语" },
  { tag: "id", label: "印尼语" },
  { tag: "ar", label: "阿拉伯语" },
  { tag: "tr", label: "土耳其语" },
];

/** 语言标签 → 中文名。认不出来就原样显示标签，保留用户手填的值。 */
export function languageLabel(tag: string): string {
  const normalized = tag.trim().toLowerCase();
  const exact = LANGUAGES.find((lang) => lang.tag === normalized);
  if (exact) return exact.label;
  // 系统的 OCR 语言是 en-us、zh-hans-cn 这类完整标签，按主语言子标签匹配。
  const primary = normalized.split(/[-_]/)[0] ?? "";
  const base = LANGUAGES.find((lang) => lang.tag === primary);
  return base ? `${base.label}（${tag}）` : tag;
}
