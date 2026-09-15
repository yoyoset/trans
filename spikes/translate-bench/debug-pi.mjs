// Debug helper: send one case through pi RPC and dump every raw event (truncated) to see why output stops.
// Usage: node debug-pi.mjs <provider/model> [caseId] [promptVersion]
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PROMPTS } from "./prompt.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const [model = "local-llama/qwen3-4b", caseId = "in-01", promptVersion = "v1"] = process.argv.slice(2);
const { system: SYSTEM_PROMPT, user: userMessage } = PROMPTS[promptVersion];
const c = JSON.parse(readFileSync(path.join(here, "cases.json"), "utf8")).cases.find((x) => x.id === caseId);
const cli = path.join(process.env.APPDATA, "npm", "node_modules", "@earendil-works", "pi-coding-agent", "dist", "cli.js");

const child = spawn(process.execPath, [cli, "--mode", "rpc", "--no-session", "--no-tools", "--no-extensions", "--no-skills",
  "--no-prompt-templates", "--no-context-files", "--offline", "--thinking", "off", "--model", model, "--system-prompt", SYSTEM_PROMPT],
  { cwd: process.env.BENCH_SANDBOX ?? here, stdio: ["pipe", "pipe", "pipe"] });
child.stderr.on("data", (d) => process.stderr.write(`[stderr] ${d}`));

let buffer = "";
child.stdout.on("data", (chunk) => {
  buffer += chunk.toString("utf8");
  let idx;
  while ((idx = buffer.indexOf("\n")) !== -1) {
    const line = buffer.slice(0, idx).replace(/\r$/, "");
    buffer = buffer.slice(idx + 1);
    let ev;
    try { ev = JSON.parse(line); } catch { console.log("[non-json]", line.slice(0, 200)); continue; }
    if (ev.type === "message_update") {
      const d = ev.assistantMessageEvent;
      console.log("update", d.type, JSON.stringify(d.delta ?? d.content ?? "").slice(0, 80));
    } else {
      console.log(ev.type, JSON.stringify(ev).slice(0, 700));
    }
    if (ev.type === "agent_settled") { child.kill(); process.exit(0); }
  }
});

child.stdin.write(JSON.stringify({ type: "prompt", message: userMessage(c) }) + "\n");
setTimeout(() => { console.log("timeout"); child.kill(); process.exit(1); }, 60000);
