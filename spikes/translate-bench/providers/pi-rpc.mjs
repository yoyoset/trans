// pi --mode rpc as a long-lived child process. Every case starts with new_session so no
// conversation context leaks between cases; the system prompt is set once via CLI flag.
import { spawn } from "node:child_process";
import path from "node:path";
const PI_CLI = path.join(process.env.APPDATA, "npm", "node_modules", "@earendil-works", "pi-coding-agent", "dist", "cli.js");

export function createPiProvider({ prompt, model, cwd, label = `pi-rpc-${model}` }) {
  let child;
  let pending = null; // { resolve, reject, started, firstTokenMs, text, toolsUsed }
  const waiters = new Map(); // response id -> resolve

  function send(cmd) {
    child.stdin.write(JSON.stringify(cmd) + "\n");
  }

  function request(cmd) {
    const id = `req-${Math.random().toString(36).slice(2)}`;
    return new Promise((resolve) => {
      waiters.set(id, resolve);
      send({ ...cmd, id });
    });
  }

  function onEvent(ev) {
    if (ev.type === "response" && ev.id && waiters.has(ev.id)) {
      waiters.get(ev.id)(ev);
      waiters.delete(ev.id);
      if (!ev.success && pending) pending.reject(new Error(ev.error));
      return;
    }
    if (!pending) return;
    if (ev.type === "message_update" && ev.assistantMessageEvent?.type === "text_delta") {
      pending.firstTokenMs ??= performance.now() - pending.started;
    }
    if (ev.type === "tool_execution_start") pending.toolsUsed = true;
    if (ev.type === "message_end" && ev.message?.role === "assistant") {
      pending.text = (ev.message.content ?? []).filter((p) => p.type === "text").map((p) => p.text).join("");
      if (ev.message.stopReason === "error") pending.error = ev.message.errorMessage ?? "assistant error";
    }
    if (ev.type === "agent_settled") {
      const p = pending;
      pending = null;
      if (p.error) p.reject(new Error(p.error));
      else p.resolve({ raw: p.text, firstTokenMs: p.firstTokenMs, totalMs: performance.now() - p.started, toolsUsed: p.toolsUsed });
    }
  }

  return {
    label,
    async start() {
      const started = performance.now();
      const args = [
        PI_CLI, "--mode", "rpc", "--no-session", "--no-tools", "--no-extensions", "--no-skills",
        "--no-prompt-templates", "--no-context-files", "--offline", "--thinking", "off",
        "--model", model, "--system-prompt", prompt.system,
      ];
      child = spawn(process.execPath, args, { cwd, stdio: ["pipe", "pipe", "pipe"] });
      child.stderr.on("data", (d) => process.stderr.write(`[pi] ${d}`));
      attachJsonl(child.stdout, (line) => {
        try { onEvent(JSON.parse(line)); } catch { /* non-JSON noise */ }
      });
      const state = await request({ type: "get_state" });
      if (!state.success) throw new Error(`pi get_state failed: ${state.error}`);
      return { startupMs: performance.now() - started, model: state.data?.model?.id };
    },
    async stop() {
      child?.stdin.end();
      child?.kill();
    },
    async translate(c) {
      await request({ type: "new_session" });
      return new Promise((resolve, reject) => {
        pending = { resolve, reject, started: performance.now(), firstTokenMs: null, text: "", toolsUsed: false };
        send({ type: "prompt", message: prompt.user(c) });
      });
    },
  };
}

// Split on LF only (pi docs: Node readline breaks on U+2028/U+2029 inside JSON strings).
function attachJsonl(stream, onLine) {
  const decoder = new TextDecoder();
  let buffer = "";
  stream.on("data", (chunk) => {
    buffer += decoder.decode(chunk, { stream: true });
    let idx;
    while ((idx = buffer.indexOf("\n")) !== -1) {
      onLine(buffer.slice(0, idx).replace(/\r$/, ""));
      buffer = buffer.slice(idx + 1);
    }
  });
}
