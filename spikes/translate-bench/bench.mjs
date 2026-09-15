// M0 translation benchmark: same cases + same prompt version across providers.
// Usage: node bench.mjs <llama|pi|opencode>... [--model provider/model] [--variant low]
//        [--prompt v1|v2] [--llama-url http://127.0.0.1:8765] [--label name]
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PROMPTS, parseOutput, validateTranslation } from "./prompt.mjs";
import { createLlamaProvider } from "./providers/llama.mjs";
import { createOpencodeProvider } from "./providers/opencode.mjs";
import { createPiProvider } from "./providers/pi-rpc.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const { cases } = JSON.parse(readFileSync(path.join(here, "cases.json"), "utf8"));

const argv = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv.splice(i, 2)[1] : fallback;
};
const model = flag("--model", "opencode/deepseek-v4-flash");
const variant = flag("--variant"); // opencode reasoning effort, e.g. low
const promptVersion = flag("--prompt", "v1");
const llamaUrl = flag("--llama-url", "http://127.0.0.1:8765");
const labelOverride = flag("--label");
const prompt = PROMPTS[promptVersion];
if (!prompt) throw new Error(`unknown prompt ${promptVersion}; expected ${Object.keys(PROMPTS).join("|")}`);

const sandbox = process.env.BENCH_SANDBOX ?? here; // agent cwd; tools are disabled anyway
// Split at the first slash only: OpenRouter ids contain more slashes (openrouter/~deepseek/x).
const slash = model.indexOf("/");
const [providerID, modelID] = [model.slice(0, slash), model.slice(slash + 1)];

const factories = {
  llama: () => createLlamaProvider({ prompt, baseUrl: llamaUrl, ...(labelOverride ? { label: labelOverride } : {}) }),
  pi: () => createPiProvider({ prompt, model, cwd: sandbox }),
  opencode: () => createOpencodeProvider({ prompt, providerID, modelID, variant }),
};

for (const name of argv) {
  const factory = factories[name];
  if (!factory) throw new Error(`unknown provider ${name}; expected ${Object.keys(factories).join("|")}`);
  await runProvider(factory());
}

async function runProvider(provider) {
  const label = `${provider.label}#${promptVersion}`;
  console.log(`\n=== ${label} ===`);
  const startInfo = await provider.start();
  if (startInfo) console.log("start:", JSON.stringify(startInfo));

  // Warm-up (not counted): first request pays connection / prompt-cache setup.
  const warm = await provider.translate(cases[0]).catch((e) => ({ error: e.message }));
  console.log(`warm-up: ${warm.error ?? `${Math.round(warm.totalMs)} ms`}`);

  const results = [];
  for (const c of cases) {
    results.push(await runCase(provider, c));
  }
  await provider.stop();

  const timed = results.filter((r) => r.totalMs != null);
  const summary = {
    provider: label,
    n: results.length,
    parsedOk: results.filter((r) => r.parsed).length,
    validOk: results.filter((r) => r.parsed && !r.invalid).length,
    toolsUsed: results.filter((r) => r.toolsUsed).length,
    totalMs: stats(timed.map((r) => r.totalMs)),
    firstTokenMs: stats(timed.map((r) => r.firstTokenMs).filter((v) => v != null)),
  };
  console.log("summary:", JSON.stringify(summary));
  const outDir = path.join(here, "out");
  mkdirSync(outDir, { recursive: true });
  writeFileSync(path.join(outDir, `${label.replaceAll(/[\\/:#]/g, "_")}.json`), JSON.stringify({ summary, startInfo, results }, null, 2));
}

async function runCase(provider, c) {
  let r;
  try {
    const out = await provider.translate(c);
    const parsed = parseOutput(out.raw);
    r = {
      id: c.id,
      totalMs: Math.round(out.totalMs),
      firstTokenMs: out.firstTokenMs == null ? null : Math.round(out.firstTokenMs),
      toolsUsed: out.toolsUsed,
      tokens: out.tokens,
      parsed: parsed.ok,
      error: parsed.ok ? undefined : parsed.error,
      translation: parsed.value?.translation,
      back: parsed.value?.back_translation,
      invalid: parsed.ok ? validateTranslation(c, parsed.value.translation) ?? undefined : undefined,
      raw: parsed.ok ? undefined : out.raw,
    };
  } catch (e) {
    r = { id: c.id, parsed: false, error: e.message };
  }
  const t = r.firstTokenMs != null ? `ttft ${r.firstTokenMs}ms ` : "";
  const flags = `${r.toolsUsed ? "TOOLS! " : ""}${r.parsed ? "" : `FAIL(${r.error}) `}${r.invalid ? `INVALID(${r.invalid}) ` : ""}`;
  console.log(`${c.id.padEnd(7)} ${t}total ${r.totalMs ?? "-"}ms ${flags}| ${r.translation ?? r.raw ?? ""}`);
  return r;
}

function stats(values) {
  if (!values.length) return null;
  const s = [...values].sort((a, b) => a - b);
  const pick = (q) => s[Math.min(s.length - 1, Math.floor(q * s.length))];
  return { min: s[0], p50: pick(0.5), p90: pick(0.9), max: s[s.length - 1] };
}
