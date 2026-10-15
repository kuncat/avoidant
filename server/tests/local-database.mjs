// Test/dev-only PostgreSQL transport. Executes the real SQL using embedded PG.
import { PGlite } from "@electric-sql/pglite";
import { readFileSync } from "node:fs";
export { localActorId as actorId } from "../../src/lib/multiplayer/local-identity.ts";
import { localActorId as actorId } from "../../src/lib/multiplayer/local-identity.ts";
export async function database() {
  const db = new PGlite();
  const migrations = new URL("../../../kuncat.com/platform/migrations/", import.meta.url);
  await db.exec(readFileSync(new URL("001_accounts.sql", migrations), "utf8"));
  await db.exec(readFileSync(new URL("002_games.sql", migrations), "utf8"));
  async function query(client, query) {
    const result = await client.query(query.query, query.params);
    return {
      fields: result.fields,
      rows: result.rows.map((row) =>
        result.fields.map((field) => {
          const value = row[field.name];
          if (value == null) return null;
          if (field.dataTypeID === 114 || field.dataTypeID === 3802) return JSON.stringify(value);
          if (typeof value === "boolean") return value ? "t" : "f";
          return String(value);
        }),
      ),
      rowCount: result.affectedRows ?? result.rows.length,
      command: query.query.trim().split(/\s/)[0],
    };
  }
  return {
    db,
    async account(token) {
      const id = actorId(token);
      await db.query("INSERT INTO kuncat.account(id) VALUES ($1) ON CONFLICT DO NOTHING", [id]);
      return id;
    },
    async fetch(request) {
      try {
        const body = await request.json();
        if (body.queries) {
          const results = await db.transaction(async (tx) => {
            const results = [];
            for (const item of body.queries) results.push(await query(tx, item));
            return results;
          });
          return Response.json({ results });
        }
        return Response.json(await query(db, body));
      } catch (error) {
        console.error("Local PostgreSQL:", error.message);
        return Response.json({ message: error.message, code: error.code }, { status: 400 });
      }
    },
  };
}
