// Re-run the current validateTranslation over saved benchmark outputs, so validator changes can be
// evaluated against past model runs without spending more CPU time on the model.
// Usage: node revalidate.mjs [filename-substring]
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { validateTranslation } from "./prompt.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const { cases } = JSON.parse(readFileSync(path.join(here, "cases.json"), "utf8"));
const byId = new Map(cases.map((c) => [c.id, c]));
const filter = process.argv[2] ?? "";
const outDir = path.join(here, "out");

for (const file of readdirSync(outDir).filter((f) => f.endsWith(".json") && f.includes(filter)).sort()) {
  const { results } = JSON.parse(readFileSync(path.join(outDir, file), "utf8"));
  const flagged = results
    .filter((r) => r.parsed)
    .map((r) => ({ id: r.id, reason: validateTranslation(byId.get(r.id), r.translation), translation: r.translation }))
    .filter((r) => r.reason);
  const parsed = results.filter((r) => r.parsed).length;
  console.log(`\n${file}: parsed ${parsed}/${results.length}, flagged ${flagged.length}`);
  for (const f of flagged) console.log(`  ${f.id.padEnd(7)} ${f.reason} | ${f.translation}`);
}
