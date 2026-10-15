import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";

// Exercise the actual room lifecycle with a storage double and no wall-clock wait.
const source = readFileSync(new URL("../src/index.ts", import.meta.url), "utf8").replace(
  /^import .*;\n/gm,
  "",
);
const moduleSource = `class DurableObject { constructor(ctx) { this.ctx = ctx; } }\n${stripTypeScriptTypes(source)}`;
const { MatchRoom } = await import(
  `data:text/javascript;base64,${Buffer.from(moduleSource).toString("base64")}`
);

test("finished room cleanup cannot be extended by repeated persistence", async () => {
  let alarm,
    deleted = false,
    closed = false;
  const storage = {
    get: async () => undefined,
    put: async () => {},
    setAlarm: async (value) => {
      alarm = value;
    },
    deleteAll: async () => {
      deleted = true;
    },
  };
  const ctx = {
    storage,
    blockConcurrencyWhile: (fn) => fn(),
    getWebSockets: () => [
      {
        close: (code) => {
          assert.equal(code, 1000);
          closed = true;
        },
      },
    ],
  };
  const room = new MatchRoom(ctx, {});
  await Promise.resolve();
  room.saved = { state: "", snapshot: {}, archived: true, map: "small", created: Date.now() };
  const result = { state: "", snapshot: { finished: true, started: true } };
  await room.persist(result);
  const deadline = alarm;
  assert.ok(deadline > Date.now());
  // Simulate an already-aged cleanup deadline, then a reconnect/rejected move.
  room.saved.cleanupAt = deadline - 30_000;
  await room.persist(result);
  assert.equal(alarm, deadline - 30_000);
  await room.alarm();
  assert.equal(deleted, true);
  assert.equal(closed, true);
  assert.equal(room.saved, undefined);
});

test("history failures prevent deletion and successful retry archives once per player", async () => {
  let deleted = false,
    fail = true,
    writes = 0;
  const ctx = {
    id: { toString: () => "match-id" },
    storage: {
      get: async () => undefined,
      put: async () => {},
      setAlarm: async () => {},
      deleteAll: async () => {
        deleted = true;
      },
    },
    blockConcurrencyWhile: (fn) => fn(),
    getWebSockets: () => [],
  };
  const room = new MatchRoom(ctx, {
    MATCHES: {
      idFromName: (name) => name,
      get: () => ({
        fetch: async () => {
          writes++;
          return new Response(null, { status: fail ? 503 : 200 });
        },
      }),
    },
  });
  await Promise.resolve();
  room.saved = {
    state: "",
    map: "small",
    created: 0,
    cleanupAt: 1,
    snapshot: {
      finished: true,
      started: true,
      finishedAt: 10,
      players: [
        { id: "a", cumulativeMs: 10 },
        { id: "b", cumulativeMs: 10 },
      ],
    },
  };
  await assert.rejects(room.alarm(), /History persistence failed/);
  assert.equal(deleted, false);
  fail = false;
  await room.archive();
  assert.equal(writes, 4);
  await room.archive();
  assert.equal(writes, 4);
  await room.alarm();
  assert.equal(deleted, true);
});
