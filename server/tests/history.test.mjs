import { test } from "node:test";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { neonConfig } from "@neondatabase/serverless";
import { database } from "./local-database.mjs";
import { saveHistory, listHistory, getHistory } from "../src/history.ts";

test("Postgres archive is atomic, idempotent, participant-only and paginated", async () => {
  const pg = await database();
  neonConfig.fetchFunction = (url, options) => pg.fetch(new Request(url, options));
  const env = { DATABASE_URL: "postgresql://test:test@db.test/kuncat" };
  try {
    const a = await pg.account("archive-a"),
      b = await pg.account("archive-b"),
      outsider = await pg.account("outsider");
    const record = {
      id: randomUUID(),
      map: "small",
      options: { mode: "territory" },
      startedAt: 1000,
      finishedAt: 3000,
      initial: { state: "private complete board", map: { shape: "frozen" } },
      replayKey: "avoidant/test/log.gz",
      replaySha256: "a".repeat(64),
      engines: ["b".repeat(64)],
      players: [a, b].map((id, slot) => ({
        id,
        name: `Player ${slot}`,
        score: slot,
        claims: slot,
        voids: 0,
        cumulativeMs: 1000,
      })),
    };
    await saveHistory(env, record);
    await saveHistory(env, record);
    assert.equal((await listHistory(env, a, null)).matches.length, 1);
    assert.equal((await listHistory(env, b, null)).matches.length, 1);
    assert.equal((await listHistory(env, outsider, null)).matches.length, 0);
    assert.equal(await getHistory(env, outsider, record.id), undefined);
    assert.deepEqual((await getHistory(env, a, record.id)).initial, record.initial);
    assert.equal((await listHistory(env, a, null)).matches[0].initial, undefined);
    const failed = {
      ...record,
      id: randomUUID(),
      replayKey: "invalid-fk",
      players: [{ ...record.players[0], id: randomUUID() }],
    };
    await assert.rejects(saveHistory(env, failed));
    assert.equal(
      (await pg.db.query("SELECT id FROM games.match WHERE id=$1", [failed.id])).rows.length,
      0,
    );
    for (let i = 0; i < 51; i++)
      await saveHistory(env, {
        ...record,
        id: randomUUID(),
        replayKey: `log-${i}`,
        finishedAt: 4000 + Math.floor(i / 2),
      });
    const first = await listHistory(env, a, null),
      second = await listHistory(env, a, first.nextCursor);
    assert.equal(first.matches.length, 50);
    assert.equal(second.matches.length, 2);
    assert.equal(second.nextCursor, null);
    assert.equal(new Set([...first.matches, ...second.matches].map((m) => m.id)).size, 52);
    await assert.rejects(listHistory(env, a, "invalid"));
    // Runtime grants deliberately exclude account and billing data.
    await pg.db.exec(
      "CREATE ROLE game_runtime; GRANT USAGE ON SCHEMA games TO game_runtime; GRANT SELECT,INSERT ON games.match,games.participant TO game_runtime; SET ROLE game_runtime;",
    );
    assert.equal((await listHistory(env, a, null)).matches.length, 50);
    await assert.rejects(pg.db.query("SELECT * FROM kuncat.account"), /permission denied/);
  } finally {
    await pg.db.close();
  }
});
