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

test("history failures retain state and schedule a retry without keeping sockets alive", async () => {
  let deleted = false,
    closed = false,
    alarm = 0;
  const ctx = {
    storage: {
      get: async () => undefined,
      setAlarm: async (value) => {
        alarm = value;
      },
      deleteAll: async () => {
        deleted = true;
      },
    },
    blockConcurrencyWhile: (fn) => fn(),
    getWebSockets: () => [
      {
        close: () => {
          closed = true;
        },
      },
    ],
  };
  const room = new MatchRoom(ctx, {});
  await Promise.resolve();
  room.saved = { snapshot: { finished: true } };
  room.archive = async () => {
    throw new Error("Database offline");
  };
  await room.alarm();
  assert.equal(deleted, false);
  assert.equal(closed, true);
  assert.ok(alarm > Date.now() + 14 * 60_000);
  room.archive = async () => {};
  await room.alarm();
  assert.equal(deleted, true);
});
