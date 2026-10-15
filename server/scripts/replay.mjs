import { gunzipSync } from "node:zlib";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";

export async function replay(compressed, loadEngine) {
  const lines = gunzipSync(compressed).toString("utf8").trim().split("\n").map(JSON.parse);
  const initial = lines.shift(),
    final = lines.pop();
  assert.equal(initial.type, "initial");
  assert.equal(initial.format, 1);
  assert.equal(final.type, "final");
  let state = initial.state;
  const engines = new Map();
  for (const event of lines) {
    assert.equal(event.type, "command");
    if (!engines.has(event.engine)) {
      const bundle = await loadEngine(event.engine);
      assert.equal(createHash("sha256").update(bundle).digest("hex"), event.engine);
      const { wasm, glue } = JSON.parse(bundle);
      const module = await import(
        `data:text/javascript;base64,${Buffer.from(glue).toString("base64")}#${event.engine}`
      );
      await module.default({ module_or_path: Buffer.from(wasm, "base64") });
      engines.set(event.engine, module);
    }
    state = JSON.parse(
      engines.get(event.engine).transition(state, JSON.stringify(event.command)),
    ).state;
  }
  assert.deepEqual(
    JSON.parse(state),
    JSON.parse(final.state),
    "Replay must reproduce the exact final private state",
  );
  return {
    moves: lines.filter((e) => e.command.kind === "move").length,
    commands: lines.length,
    state: JSON.parse(state),
  };
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [base, id] = process.argv.slice(2);
  if (!base || !id || !process.env.KUNCAT_ACCESS_TOKEN)
    throw new Error("Usage: KUNCAT_ACCESS_TOKEN=… node scripts/replay.mjs <server-url> <match-id>");
  const headers = {
    Authorization: `Bearer ${process.env.KUNCAT_ACCESS_TOKEN}`,
    Origin: process.env.KUNCAT_ORIGIN ?? "https://games.kuncat.com",
  };
  async function get(path) {
    const response = await fetch(`${base.replace(/\/$/, "")}/history/${id}/${path}`, { headers });
    if (!response.ok) throw new Error(`Replay fetch failed: ${response.status}`);
    return response;
  }
  const response = await get("replay");
  const bytes = Buffer.from(await response.arrayBuffer());
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    response.headers.get("X-Replay-SHA256"),
  );
  const result = await replay(bytes, async (hash) => (await get(`engines/${hash}`)).text());
  console.log(
    `Verified ${result.commands} commands (${result.moves} moves); final state matches exactly.`,
  );
}
