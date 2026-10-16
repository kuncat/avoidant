import { test as nodeTest, after } from "node:test";
const test = (name, run) => nodeTest(name, { timeout: 15_000 }, run);
import assert from "node:assert/strict";
import { replay } from "../scripts/replay.mjs";
import { database } from "./local-database.mjs";
const databases = new Map();
after(async () => {
  for (const pg of databases.values()) await (await pg).db.close();
});
import { Miniflare } from "miniflare";
import { readdirSync, mkdtempSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

function runtime(requireEntitlement = "false", persist, authTtl = 900_000) {
  const key = persist ?? Symbol();
  if (!databases.has(key)) databases.set(key, database());
  const pg = databases.get(key);
  const mf = new Miniflare({
    modules: [
      { type: "ESModule", path: resolve("dist/index.js") },
      ...readdirSync("dist")
        .filter((f) => f.endsWith(".wasm"))
        .map((f) => ({ type: "CompiledWasm", path: resolve("dist", f) })),
    ],
    compatibilityDate: "2026-06-01",
    durableObjectsPersist: persist,
    r2Buckets: ["REPLAYS"],
    r2Persist: persist ? `${persist}/r2` : undefined,
    durableObjects: { MATCHES: { className: "MatchRoom", useSQLite: true } },
    bindings: {
      DATABASE_URL: "postgresql://test:test@db.test/kuncat",
      ALLOWED_ORIGINS: "https://games.example",
      ACCOUNT_API: "https://accounts.example",
      REQUIRE_ENTITLEMENT: requireEntitlement,
    },
    outboundService: async (request) => {
      if (new URL(request.url).pathname === "/sql") return (await pg).fetch(request);
      const token = request.headers.get("Authorization");
      if (!["Bearer a", "Bearer b", "Bearer c"].includes(token))
        return Response.json({ error: "Unauthorized" }, { status: 401 });
      return Response.json({
        id: await (await pg).account(token.slice(7)),
        capabilities: [],
        expiresAt: Date.now() + authTtl,
      });
    },
  });
  const dispose = mf.dispose.bind(mf);
  mf.dispose = async () => {
    await dispose();
    if (!persist) {
      await (await pg).db.close();
      databases.delete(key);
    }
  };
  return mf;
}
const headers = (actor = "a") => ({
  Origin: "https://games.example",
  Authorization: `Bearer ${actor}`,
  "Content-Type": "application/json",
});
async function post(mf, path, body, actor = "a") {
  return mf.dispatchFetch(`https://game.example${path}`, {
    method: "POST",
    headers: headers(actor),
    body: JSON.stringify(body),
  });
}
async function connect(mf, id, actor, name) {
  const res = await post(mf, `/matches/${id}/connect`, { name }, actor);
  assert.equal(res.status, 200);
  const { ticket, preview } = await res.json();
  assert.deepEqual(Object.keys(preview).sort(), ["bordering", "map", "mode"]);
  assert.equal(typeof preview.map, "string");
  assert.ok(["collaborative", "territory"].includes(preview.mode));
  assert.equal(typeof preview.bordering, "boolean");
  const upgrade = await mf.dispatchFetch(
    `https://game.example/matches/${id}/socket?ticket=${ticket}`,
    { headers: { Origin: "https://games.example", Upgrade: "websocket" } },
  );
  assert.equal(upgrade.status, 101);
  const ws = upgrade.webSocket;
  const messages = [];
  const waiters = [];
  ws.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    messages.push(message);
    for (const wake of waiters.splice(0)) wake();
  });
  ws.accept();
  async function next(predicate) {
    const until = Date.now() + 5000;
    while (Date.now() < until) {
      const index = messages.findIndex(predicate);
      if (index >= 0) return messages.splice(index, 1)[0];
      await new Promise((resolve) => {
        waiters.push(resolve);
        setTimeout(resolve, 50);
      });
    }
    throw new Error("Timed out waiting for snapshot");
  }
  return { ws, next, ticket };
}

test("trusted territory lifecycle, redaction, membership, tickets and idempotency", async () => {
  const mf = runtime();
  try {
    const created = await post(mf, "/matches", {
      map: "small",
      mode: "territory",
      bordering: false,
      turnBased: false,
      floodLimit: 3,
    });
    assert.equal(created.status, 201);
    const { id } = await created.json();
    for (const name of ["", " ", "a".repeat(33), "line\nbreak", 42]) {
      assert.equal((await post(mf, `/matches/${id}/connect`, { name }, "b")).status, 400);
    }
    const a = await connect(mf, id, "a", "  আশা  ");
    const b = await connect(mf, id, "b", "Bela");
    a.ws.send(JSON.stringify({ kind: "start" }));
    const initial = (await a.next((m) => m.type === "snapshot" && m.snapshot.started)).snapshot;
    assert.equal(initial.players.length, 2);
    assert.deepEqual(
      initial.players.map((p) => p.name),
      ["আশা", "Bela"],
    );
    for (const c of initial.cells.filter((c) => !c.revealed)) {
      assert.equal(c.isVoid, undefined);
      assert.equal(c.count, undefined);
    }
    const replay = await mf.dispatchFetch(
      `https://game.example/matches/${id}/socket?ticket=${a.ticket}`,
      { headers: { Origin: "https://games.example", Upgrade: "websocket" } },
    );
    assert.equal(replay.status, 401);
    const cell = initial.cells.findIndex((c) => !c.revealed);
    const action = { kind: "move", sequence: 1, action: { kind: "select", cell } };
    a.ws.send(JSON.stringify(action));
    const moved = (
      await a.next((m) => m.type === "snapshot" && m.snapshot.players[0].sequence === 1)
    ).snapshot;
    assert.equal(moved.cells[cell].revealed, true);
    assert.ok(moved.players[0].claims <= 2);
    a.ws.send(JSON.stringify(action));
    const repeated = (
      await a.next((m) => m.type === "snapshot" && m.snapshot.players[0].sequence === 1)
    ).snapshot;
    assert.equal(repeated.revision, moved.revision);
    assert.equal(repeated.players[0].score, moved.players[0].score);
    a.ws.close();
    const reconnected = await connect(mf, id, "a");
    const restored = (await reconnected.next((m) => m.type === "snapshot")).snapshot;
    assert.equal(restored.players[0].sequence, 1);
    b.ws.close();
    reconnected.ws.close();
  } finally {
    await mf.dispose();
  }
});

test("login, origin and entitlement failures deny creation", async () => {
  const mf = runtime("true");
  try {
    assert.equal((await post(mf, "/matches", {}, "invalid")).status, 401);
    assert.equal((await post(mf, "/matches", {})).status, 403);
    const bad = await mf.dispatchFetch("https://game.example/matches", {
      method: "POST",
      headers: { Origin: "https://attacker.example" },
    });
    assert.equal(bad.status, 403);
  } finally {
    await mf.dispose();
  }
});

test("room state survives runtime restart", async () => {
  const persist = mkdtempSync(`${tmpdir()}/avoidant-room-`);
  let mf = runtime("false", persist);
  try {
    const created = await post(mf, "/matches", {
      map: "medium",
      mode: "territory",
      bordering: false,
      turnBased: true,
      floodLimit: null,
    });
    const { id } = await created.json();
    const a = await connect(mf, id, "a"),
      b = await connect(mf, id, "b");
    a.ws.send(JSON.stringify({ kind: "start" }));
    const before = (await a.next((m) => m.type === "snapshot" && m.snapshot.started)).snapshot;
    a.ws.close();
    b.ws.close();
    await mf.dispose();
    mf = runtime("false", persist);
    const restored = await connect(mf, id, "a");
    const after = (await restored.next((m) => m.type === "snapshot")).snapshot;
    assert.ok(after.players[0].cumulativeMs >= before.players[0].cumulativeMs);
    before.players[0].cumulativeMs = after.players[0].cumulativeMs;
    assert.deepEqual(after, before);
    restored.ws.close();
  } finally {
    await mf.dispose();
    rmSync(persist, { recursive: true, force: true });
  }
});

test("expired authentication cannot move on an existing socket", async () => {
  const mf = runtime("false", undefined, 500);
  try {
    const created = await post(mf, "/matches", {
      map: "small",
      mode: "territory",
      bordering: false,
      turnBased: true,
      floodLimit: 0,
    });
    const { id } = await created.json();
    const a = await connect(mf, id, "a");
    await new Promise((r) => setTimeout(r, 600));
    const closed = new Promise((resolve) =>
      a.ws.addEventListener("close", resolve, { once: true }),
    );
    a.ws.send(JSON.stringify({ kind: "start" }));
    assert.equal((await closed).code, 4001);
  } finally {
    await mf.dispose();
  }
});

test("flood limits are validated and every catalog preset creates a match", async () => {
  const mf = runtime();
  const options = {
    map: "large",
    mode: "territory",
    bordering: true,
    turnBased: false,
    floodLimit: 0,
  };
  try {
    for (const floodLimit of [-1, 1.5, "infinite", 4097])
      assert.equal((await post(mf, "/matches", { ...options, floodLimit })).status, 400);
    for (const map of Object.keys(JSON.parse(readFileSync("src/catalog.json", "utf8"))))
      assert.equal((await post(mf, "/matches", { ...options, map })).status, 201);
  } finally {
    await mf.dispose();
  }
});

test("collaborative rooms authenticate teammates and share authoritative state without ownership", async () => {
  const mf = runtime();
  try {
    const response = await post(mf, "/matches", {
      map: "small",
      mode: "collaborative",
      bordering: true,
      turnBased: true,
      floodLimit: 0,
    });
    assert.equal(response.status, 201);
    const { id } = await response.json();
    const a = await connect(mf, id, "a");
    const b = await connect(mf, id, "b");
    a.ws.send(JSON.stringify({ kind: "start" }));
    const initial = (await b.next((m) => m.type === "snapshot" && m.snapshot.started)).snapshot;
    assert.equal(initial.turn, null);
    assert.equal(initial.options.bordering, false);
    const cell = initial.cells.findIndex((c) => !c.revealed);
    b.ws.send(JSON.stringify({ kind: "move", sequence: 1, action: { kind: "select", cell } }));
    const changed = (
      await a.next((m) => m.type === "snapshot" && m.snapshot.players[1]?.sequence === 1)
    ).snapshot;
    assert.equal(changed.cells[cell].revealed, true);
    assert.ok(changed.cells.every((c) => c.owner === null));
    const c = await connect(mf, id, "c");
    const joined = (await c.next((m) => m.type === "snapshot" && m.snapshot.players.length === 3))
      .snapshot;
    assert.equal(joined.cells[cell].revealed, true);
    assert.ok(
      joined.cells
        .filter((c) => !c.revealed)
        .every((c) => c.isVoid === undefined && c.count === undefined),
    );
    for (const socket of [a, b, c]) socket.ws.close();
  } finally {
    await mf.dispose();
  }
});

test("host durations are bounded and disabled turn timers retain only match expiry", async () => {
  const mf = runtime();
  try {
    const options = {
      map: "small",
      mode: "territory",
      bordering: false,
      turnBased: true,
      floodLimit: 0,
      matchMinutes: 1440,
      turnSeconds: null,
    };
    for (const patch of [
      { matchMinutes: 1441 },
      { matchMinutes: 0 },
      { matchMinutes: 1.5 },
      { turnSeconds: 0 },
      { turnSeconds: 86401 },
    ]) {
      assert.equal((await post(mf, "/matches", { ...options, ...patch })).status, 400);
    }
    const created = await post(mf, "/matches", options);
    assert.equal(created.status, 201);
    const { id } = await created.json();
    const a = await connect(mf, id, "a", "Asha");
    const b = await connect(mf, id, "b", "Bela");
    const before = Date.now();
    a.ws.send(JSON.stringify({ kind: "start" }));
    const { snapshot } = await a.next((m) => m.type === "snapshot" && m.snapshot.started);
    assert.equal(snapshot.options.matchMinutes, 1440);
    assert.equal(snapshot.options.turnSeconds, null);
    assert.equal(snapshot.turnDeadline, 0);
    assert.ok(snapshot.deadline >= before + 86400000);
    assert.ok(snapshot.deadline <= Date.now() + 86400000);
    a.ws.close();
    b.ws.close();
  } finally {
    await mf.dispose();
  }
});

test("completed times survive restart in private history", async () => {
  const persist = mkdtempSync(`${tmpdir()}/avoidant-history-`);
  let mf = runtime("false", persist);
  try {
    const { id } = await (
      await post(mf, "/matches", {
        map: "small",
        mode: "territory",
        bordering: false,
        turnBased: true,
        floodLimit: 0,
        turnSeconds: null,
      })
    ).json();
    const a = await connect(mf, id, "a", "Asha");
    const b = await connect(mf, id, "b", "Bela");
    a.ws.send(JSON.stringify({ kind: "start" }));
    await a.next((m) => m.type === "snapshot" && m.snapshot.started);
    a.ws.send(JSON.stringify({ kind: "move", sequence: 1, action: { kind: "select", cell: 0 } }));
    await b.next((m) => m.type === "snapshot" && m.snapshot.turn === 1);
    b.ws.send(JSON.stringify({ kind: "move", sequence: 1, action: { kind: "pass" } }));
    await a.next((m) => m.type === "snapshot" && m.snapshot.players[1]?.sequence === 1);
    await new Promise((resolve) => setTimeout(resolve, 450));
    a.ws.send(JSON.stringify({ kind: "move", sequence: 2, action: { kind: "pass" } }));
    const { snapshot } = await b.next((m) => m.type === "snapshot" && m.snapshot.finished);
    assert.equal(
      snapshot.players.reduce((sum, p) => sum + p.cumulativeMs, 0),
      snapshot.finishedAt - snapshot.startedAt,
    );
    const replayResponse = await mf.dispatchFetch(`https://game.example/history/${id}/replay`, {
      headers: headers("a"),
    });
    assert.equal(replayResponse.status, 200);
    const compressed = Buffer.from(await replayResponse.arrayBuffer());
    const verified = await replay(compressed, async (hash) => {
      const response = await mf.dispatchFetch(
        `https://game.example/history/${id}/engines/${hash}`,
        { headers: headers("b") },
      );
      assert.equal(response.status, 200);
      return response.text();
    });
    assert.equal(verified.moves, 3);
    assert.equal(
      (
        await mf.dispatchFetch(`https://game.example/history/${id}/replay`, {
          headers: headers("c"),
        })
      ).status,
      404,
    );
    const initial = await (
      await mf.dispatchFetch(`https://game.example/history/${id}/initial`, {
        headers: headers("a"),
      })
    ).json();
    assert.ok(JSON.parse(initial.state).voids.length > 0);
    assert.ok(initial.map.shape);
    a.ws.close();
    b.ws.close();
    await mf.dispose();
    mf = runtime("false", persist);
    for (const actor of ["a", "b", "c"]) {
      const response = await mf.dispatchFetch("https://game.example/history", {
        headers: headers(actor),
      });
      assert.equal(response.status, 200);
      const { matches } = await response.json();
      assert.equal(matches.length, actor === "c" ? 0 : 1);
      if (matches.length) {
        assert.equal(matches[0].id, id);
        assert.deepEqual(
          matches[0].players.map((p) => p.cumulativeMs),
          snapshot.players.map((p) => p.cumulativeMs),
        );
        assert.equal(matches[0].cells, undefined);
      }
    }
    assert.equal(
      (await mf.dispatchFetch("https://game.example/history", { headers: headers("invalid") }))
        .status,
      401,
    );
  } finally {
    await mf.dispose();
    rmSync(persist, { recursive: true, force: true });
  }
});
