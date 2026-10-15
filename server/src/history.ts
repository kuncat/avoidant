import { neon } from "@neondatabase/serverless";
import type { Snapshot } from "./rules";

export interface HistoryEnv {
  DATABASE_URL: string;
  REPLAYS: R2Bucket;
}
export interface ArchiveRecord {
  id: string;
  map: string;
  options: Snapshot["options"];
  startedAt: number | null;
  finishedAt: number;
  players: Snapshot["players"];
  initial: unknown;
  replayKey: string | null;
  replaySha256: string | null;
  engines: string[];
}
export async function saveHistory(env: HistoryEnv, record: ArchiveRecord) {
  const sql = neon(env.DATABASE_URL);
  // A single transaction publishes the match and every participant together.
  await sql.transaction([
    sql`INSERT INTO games.match (id, product, map, options, initial_state, started_at_ms, finished_at_ms, replay_key, replay_sha256, engines)
      VALUES (${record.id}::uuid, 'avoidant', ${record.map}, ${JSON.stringify(record.options)}::jsonb,
        ${JSON.stringify(record.initial)}::jsonb, ${record.startedAt}, ${record.finishedAt},
        ${record.replayKey}, ${record.replaySha256}, ${JSON.stringify(record.engines)}::jsonb)
      ON CONFLICT (id) DO NOTHING`,
    sql`INSERT INTO games.participant (match_id, account_id, slot, name, score, claims, voids, cumulative_ms)
      SELECT ${record.id}::uuid, (p->>'id')::uuid, (ordinality - 1)::int, coalesce(p->>'name',''),
        (p->>'score')::int, (p->>'claims')::int, (p->>'voids')::int, (p->>'cumulativeMs')::bigint
      FROM jsonb_array_elements(${JSON.stringify(record.players)}::jsonb) WITH ORDINALITY AS players(p, ordinality)
      ON CONFLICT (match_id, account_id) DO NOTHING`,
  ]);
}
export async function listHistory(env: HistoryEnv, actor: string, cursor: string | null) {
  let beforeTime = Number.MAX_SAFE_INTEGER,
    beforeId = "ffffffff-ffff-ffff-ffff-ffffffffffff";
  if (cursor) {
    const parts = /^(\d{1,16}):([a-f0-9-]{36})$/.exec(cursor);
    if (!parts || !Number.isSafeInteger(Number(parts[1]))) throw new Error("Invalid cursor");
    beforeTime = Number(parts[1]);
    beforeId = parts[2];
  }
  const sql = neon(env.DATABASE_URL);
  const rows = await sql`SELECT m.id, m.map, m.options, m.started_at_ms::float8 AS "startedAt",
      m.finished_at_ms::float8 AS "finishedAt", (m.replay_key IS NOT NULL) AS "replayAvailable",
      (SELECT jsonb_agg(jsonb_build_object('id',p.account_id,'name',p.name,'score',p.score,
          'claims',p.claims,'voids',p.voids,'cumulativeMs',p.cumulative_ms) ORDER BY p.slot)
       FROM games.participant p WHERE p.match_id=m.id) AS players
    FROM games.match m JOIN games.participant mine ON mine.match_id=m.id
    WHERE mine.account_id=${actor}::uuid AND m.product='avoidant'
      AND (m.finished_at_ms,m.id) < (${beforeTime}::bigint,${beforeId}::uuid)
    ORDER BY m.finished_at_ms DESC,m.id DESC LIMIT 51`;
  const matches = rows.slice(0, 50);
  const last = matches.at(-1);
  return { matches, nextCursor: rows.length > 50 ? `${last!.finishedAt}:${last!.id}` : null };
}
export async function getHistory(env: HistoryEnv, actor: string, id: string) {
  const sql = neon(env.DATABASE_URL);
  const rows = await sql`SELECT m.initial_state AS initial, m.replay_key AS "replayKey",
    m.replay_sha256 AS "replaySha256", m.engines FROM games.match m
    JOIN games.participant p ON p.match_id=m.id
    WHERE m.id=${id}::uuid AND p.account_id=${actor}::uuid AND m.product='avoidant'`;
  return rows[0];
}
