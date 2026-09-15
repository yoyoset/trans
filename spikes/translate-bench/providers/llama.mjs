// llama-server (OpenAI-compatible) with SSE streaming, so time-to-first-token is measurable.
// temperature 0 by default: benchmark runs must be reproducible (v1 gave different in-06 output across runs at 0.2).
export function createLlamaProvider({ prompt, baseUrl = "http://127.0.0.1:8765", label = "llama-cpu-qwen3-4b", temperature = 0 }) {
  return {
    label,
    async start() {},
    async stop() {},
    async translate(c) {
      const started = performance.now();
      const res = await fetch(`${baseUrl}/v1/chat/completions`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          stream: true,
          temperature,
          max_tokens: 256,
          messages: [
            { role: "system", content: prompt.system },
            { role: "user", content: prompt.user(c) },
          ],
        }),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}: ${await res.text()}`);

      let firstTokenMs = null;
      let text = "";
      for await (const data of sseData(res.body)) {
        if (data === "[DONE]") break;
        const delta = JSON.parse(data).choices?.[0]?.delta?.content;
        if (delta) {
          firstTokenMs ??= performance.now() - started;
          text += delta;
        }
      }
      return { raw: text, firstTokenMs, totalMs: performance.now() - started, toolsUsed: false };
    },
  };
}

async function* sseData(body) {
  const decoder = new TextDecoder();
  let buffer = "";
  for await (const chunk of body) {
    buffer += decoder.decode(chunk, { stream: true });
    let idx;
    while ((idx = buffer.indexOf("\n")) !== -1) {
      const line = buffer.slice(0, idx).replace(/\r$/, "");
      buffer = buffer.slice(idx + 1);
      if (line.startsWith("data:")) yield line.slice(5).trim();
    }
  }
}
