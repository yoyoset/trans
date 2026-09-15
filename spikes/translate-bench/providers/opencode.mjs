// opencode serve over HTTP. The message endpoint is synchronous, so only total latency is
// measured (no first-token time). All tools are disabled per request using the server's tool id list.
export function createOpencodeProvider({ prompt, baseUrl = "http://127.0.0.1:4096", providerID, modelID, variant, label = `opencode-serve-${providerID}/${modelID}${variant ? `@${variant}` : ""}` }) {
  let toolsOff = {};

  async function call(method, route, body) {
    const res = await fetch(`${baseUrl}${route}`, {
      method,
      headers: { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await res.text();
    if (!res.ok) throw new Error(`${method} ${route} -> HTTP ${res.status}: ${text.slice(0, 300)}`);
    return text ? JSON.parse(text) : null;
  }

  return {
    label,
    async start() {
      const started = performance.now();
      const ids = await call("GET", "/experimental/tool/ids");
      toolsOff = Object.fromEntries(ids.map((id) => [id, false]));
      return { startupMs: performance.now() - started, toolCount: ids.length };
    },
    async stop() {},
    async translate(c) {
      const started = performance.now();
      const session = await call("POST", "/session", { title: `bench ${c.id}` });
      const sessionMs = performance.now() - started;
      const reply = await call("POST", `/session/${session.id}/message`, {
        model: { providerID, modelID },
        ...(variant ? { variant } : {}),
        system: prompt.system,
        tools: toolsOff,
        parts: [{ type: "text", text: prompt.user(c) }],
      });
      const parts = reply.parts ?? [];
      const raw = parts.filter((p) => p.type === "text").map((p) => p.text).join("");
      const toolsUsed = parts.some((p) => p.type === "tool");
      const error = reply.info?.error;
      if (error && !raw) throw new Error(JSON.stringify(error).slice(0, 300));
      await call("DELETE", `/session/${session.id}`).catch(() => {});
      // tokens.input reveals how much hidden agent prompt opencode adds; tokens.reasoning reveals thinking cost.
      const tokens = reply.info?.tokens;
      return { raw, firstTokenMs: null, totalMs: performance.now() - started, sessionMs, toolsUsed, tokens };
    },
  };
}
