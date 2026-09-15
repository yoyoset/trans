// M0 translation benchmark: same cases + same prompt envelope across providers.
// Usage: node bench.mjs <llama|pi|opencode>... [--model provider/model]
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseOutput } from "./prompt.mjs";
import { createLlamaProvider } from "./providers/llama.mjs";
import { createOpencodeProvider } from "./providers/opencode.mjs";
import { createPiProvider } from "./providers/pi-rpc.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const { cases } = JSON.parse(readFileSync(path.join(here, "cases.json"), "utf8"));

const argv = process.argv.slice(2);
const modelFlag = argv.indexOf("--model");
const model = modelFlag >= 0 ? argv.splice(modelFlag, 2)[1] : "opencode/deepseek-v4-flash";
const variantFlag = argv.indexOf("--variant");
const variant = variantFlag >= 0 ? argv.splice(variantFlag, 2)[1] : undefined; // opencode reasoning effort, e.g. low
const sandbox = process.env.BENCH_SANDBOX ?? here; // agent cwd; tools are disabled anyway
// Split at the first slash only: OpenRouter ids contain more slashes (openrouter/~deepseek/x).
const slash = model.indexOf("/");
const [providerID, modelID] = [model.slice(0, slash), model.slice(slash + 1)];

const factories = {
  llama: () => createLlamaProvider(),
  pi: () => createPiProvider({ model, cwd: sandbox }),
  opencode: () => createOpencodeProvider({ providerID, modelID, variant }),
};

for (const name of argv) {
  const factory = factories[name];
  if (!factory) throw new Error(`unknown provider ${name}; expected ${Object.keys(factories).join("|")}`);
  await runProvider(factory());
}

async function runProvider(provider) {
  console.log(`\n=== ${provider.label} ===`);
  const startInfo = await provider.start();
  if (startInfo) console.log("start:", JSON.stringify(startInfo));

  // Warm-up (not counted): first request pays connection / prompt-cache setup.
  const warm = await provider.translate(cases[0]).catch((e) => ({ error: e.message }));
  console.log(`warm-up: ${warm.error ?? `${Math.round(warm.totalMs)} ms`}`);

  const results = [];
  for (const c of cases) {
    let r;
    try {
      const out = await provider.translate(c);
      const parsed = parseOutput(out.raw);
      r = { id: c.id, ...round(out), parsed: parsed.ok, error: parsed.ok ? undefined : parsed.error, translation: parsed.value?.translation, back: parsed.value?.back_translation, raw: parsed.ok ? undefined : out.raw };
    } catch (e) {
      r = { id: c.id, parsed: false, error: e.message };
    }
    results.push(r);
    const t = r.firstTokenMs != null ? `ttft ${r.firstTokenMs}ms ` : "";
    console.log(`${c.id.padEnd(7)} ${t}total ${r.totalMs ?? "-"}ms ${r.toolsUsed ? "TOOLS! " : ""}${r.parsed ? "" : `FAIL(${r.error}) `}| ${r.translation ?? r.raw ?? ""}`);
  }
  await provider.stop();

  const ok = results.filter((r) => r.totalMs != null);
  const summary = {
    provider: provider.label,
    n: results.length,
    parsedOk: results.filter((r) => r.parsed).length,
    toolsUsed: results.filter((r) => r.toolsUsed).length,
    totalMs: stats(ok.map((r) => r.totalMs)),
    firstTokenMs: stats(ok.map((r) => r.firstTokenMs).filter((v) => v != null)),
  };
  console.log("summary:", JSON.stringify(summary));
  const outDir = path.join(here, "out");
  mkdirSync(outDir, { recursive: true });
  writeFileSync(path.join(outDir, `${provider.label.replaceAll(/[\\/:]/g, "_")}.json`), JSON.stringify({ summary, startInfo, results }, null, 2));
}

function round(o) {
  return { ...o, raw: o.raw, totalMs: Math.round(o.totalMs), firstTokenMs: o.firstTokenMs == null ? null : Math.round(o.firstTokenMs), sessionMs: o.sessionMs == null ? undefined : Math.round(o.sessionMs) };
}

function stats(values) {
  if (!values.length) return null;
  const s = [...values].sort((a, b) => a - b);
  const pick = (q) => s[Math.min(s.length - 1, Math.floor(q * s.length))];
  return { min: s[0], p50: pick(0.5), p90: pick(0.9), max: s[s.length - 1] };
}
