import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";

// Exercise the actual room lifecycle with a storage double and no wall-clock wait.
const source = readFileSync(new URL("../src/index.ts", import.meta.url), "utf8").replace(
  /^import .*;\n/gm,
  "",
);
const sharedModule = new URL("../../../kuncat.com/multiplayer/server.ts", import.meta.url).href;
const moduleSource = `import { directory, openGames, publish, metric } from ${JSON.stringify(sharedModule)};\nclass DurableObject { constructor(ctx) { this.ctx = ctx; } }\n${stripTypeScriptTypes(source)}`;
const { MatchRoom, default: worker } = await import(
  `data:text/javascript;base64,${Buffer.from(moduleSource).toString("base64")}`
);

test("finished room cleanup cannot be extended by repeated persistence", async () => {
  let alarm,
    deleted = false,
    closed = false;
  const storage = {
    get: async () => undefined,
    delete: async () => {},
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
      delete: async () => {},
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

test("retirement retries preserve the ID after playable state is removed", async () => {
  const data = new Map();
  let alarm = 0,
    calls = 0;
  const id = crypto.randomUUID();
  const ctx = {
    storage: {
      get: async (key) => data.get(key),
      put: async (key, value) => data.set(key, value),
      delete: async (key) => data.delete(key),
      deleteAll: async () => data.clear(),
      setAlarm: async (value) => {
        alarm = value;
      },
    },
    blockConcurrencyWhile: (fn) => fn(),
    getWebSockets: () => [],
  };
  const room = new MatchRoom(ctx, {
    INVITATIONS: {
      retire: async (session) => {
        assert.equal(session, id);
        assert.equal(data.has("match"), false);
        if (++calls === 1) throw Error("Registry offline");
      },
    },
  });
  await Promise.resolve();
  room.saved = { id, snapshot: { started: false } };
  data.set("match", room.saved);
  await room.alarm();
  assert.equal(room.saved, undefined);
  assert.equal(data.get("retiring"), id);
  assert.ok(alarm > Date.now());
  await room.alarm();
  assert.equal(calls, 2);
  assert.equal(data.size, 0);
});


test('sharing-code exchange accepts packaged displays and rejects unknown or finished games', async () => {
  const room = new MatchRoom({
    storage: { get: async () => undefined, put: async () => {} },
    blockConcurrencyWhile: fn => fn(), getWebSockets: () => [],
  }, {});
  await Promise.resolve();
  room.saved = { id: crypto.randomUUID(), snapshot: { finished: false } };
  const env = {
    ALLOWED_ORIGINS: 'https://games.test',
    INVITATIONS: { resolve: async code => code === 'SHARECODE' ? { session: room.saved.id } : null },
    MATCHES: { idFromName: x => x, get: id => id.startsWith('watch-quota:') ? { fetch: async () => Response.json({}) } : room },
  };
  const pair = code => worker.fetch(new Request('https://api.test/pair', {
    method: 'POST', headers: { Origin: 'null' }, body: JSON.stringify({ code }),
  }), env);
  const first = await pair('SHARECODE');
  assert.equal(first.status, 200);
  assert.equal(first.headers.get('Access-Control-Allow-Origin'), 'null');
  const second = await pair('SHARECODE');
  assert.equal(second.status, 200);
  assert.notEqual((await first.json()).capability, (await second.json()).capability);
  assert.equal((await pair('BADCODE')).status, 404);
  room.saved.snapshot.finished = true;
  assert.equal((await pair('SHARECODE')).status, 410);
});
