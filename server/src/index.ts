import { DurableObject } from "cloudflare:workers";
import { saveHistory, listHistory, getHistory, type HistoryEnv } from "./history";
import { engineHash, engineBundle } from "../rules/replay-engine";
import catalog from "./catalog.json";
import { rules, type Result, type Snapshot } from "./rules";

interface Env extends HistoryEnv {
  MATCHES: DurableObjectNamespace<MatchRoom>;
  INVITATIONS?: {
    retire(session: string): Promise<void>;
    register(session: string): Promise<{ code: string; url: string }>;
    resolve(code: string): Promise<{ session: string } | null>;
  };
  ALLOWED_ORIGINS: string;
  ACCOUNT_API: string;
  REQUIRE_ENTITLEMENT: string;
}
interface Saved {
  state: string;
  snapshot: Snapshot;
  map: keyof typeof catalog;
  created: number;
  id?: string;
  archived?: boolean;
  initial?: { format: 1; state: string; map: unknown; engine: string };
  eventCount?: number;
  engines?: string[];
  replayKey?: string;
  replaySha256?: string;
  cleanupAt?: number;
}
interface Attachment {
  actor: string;
  expires: number;
  window: number;
  messages: number;
}
const json = (data: unknown, status = 200) =>
  Response.json(data, { status, headers: { "Cache-Control": "no-store" } });
const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/;

async function boundedBody(request: Request, limit: number): Promise<string | null> {
  const reader = request.body?.getReader();
  if (!reader) return "";
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit) {
        await reader.cancel();
        return null;
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const origin = request.headers.get("Origin") ?? "";
    if (!env.ALLOWED_ORIGINS.split(",").includes(origin))
      return json({ error: "Origin not allowed" }, 403);
    const cors = {
      "Access-Control-Allow-Origin": origin,
      "Access-Control-Allow-Headers": "Authorization, Content-Type",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      Vary: "Origin",
    };
    if (request.method === "OPTIONS") return new Response(null, { headers: cors });
    const reply = (data: unknown, status = 200) =>
      Response.json(data, { status, headers: { ...cors, "Cache-Control": "no-store" } });
    let response: Response;
    try {
      const url = new URL(request.url);
      const historyPath =
        /^\/history\/([a-f0-9-]{36})\/(replay|initial|engines\/([a-f0-9]{64}))$/.exec(url.pathname);
      const match = /^\/matches\/([^/]+)(?:\/(connect|socket))?$/.exec(url.pathname);
      if (match && match[2] === "connect" && /^[A-Za-z]{4,32}$/.test(match[1])) {
        const resolved = await env.INVITATIONS?.resolve(match[1].toUpperCase());
        if (!resolved) return reply({ error: "Invitation not found" }, 404);
        match[1] = resolved.session;
      }
      if (match && uuid.test(match[1]) && match[2] === "socket" && request.method === "GET") {
        // The room consumes a short-lived single-use ticket; never put bearer tokens in URLs.
        return await env.MATCHES.get(env.MATCHES.idFromName(match[1])).fetch(request);
      }
      if (
        !(
          request.method === "GET" &&
          (url.pathname === "/history" || (historyPath && uuid.test(historyPath[1])))
        ) &&
        (request.method !== "POST" ||
          (url.pathname !== "/matches" &&
            !(match && uuid.test(match[1]) && match[2] === "connect")))
      )
        return reply({ error: "Not found" }, 404);
      const authorization = request.headers.get("Authorization");
      if (!authorization?.startsWith("Bearer ")) return reply({ error: "Login required" }, 401);
      const accountResponse = await fetch(`${env.ACCOUNT_API}/v1/me`, {
        headers: { Authorization: authorization },
        signal: AbortSignal.timeout(8000),
      });
      if (!accountResponse.ok)
        return reply(
          { error: "Account verification failed" },
          accountResponse.status === 401 ? 401 : 503,
        );
      const account = (await accountResponse.json()) as {
        id: string;
        capabilities: string[];
        expiresAt: number;
      };
      if (
        typeof account.id !== "string" ||
        !Array.isArray(account.capabilities) ||
        !Number.isFinite(account.expiresAt) ||
        account.expiresAt <= Date.now()
      )
        throw new Error("Invalid account response");
      if (url.pathname === "/history" || historyPath) {
        try {
          if (!historyPath)
            return reply(await listHistory(env, account.id, url.searchParams.get("cursor")));
          const record = await getHistory(env, account.id, historyPath[1]);
          if (!record) return reply({ error: "Match not found" }, 404);
          if (historyPath[2] === "initial") return reply(record.initial);
          const key = historyPath[3]
            ? record.engines.includes(historyPath[3])
              ? `engines/${historyPath[3]}.json`
              : null
            : record.replayKey;
          if (!key) return reply({ error: "Replay not available" }, 404);
          const object = await env.REPLAYS.get(key);
          if (!object) return reply({ error: "Replay unavailable" }, 503);
          return new Response(object.body, {
            headers: {
              ...cors,
              "Cache-Control": "private, no-store",
              "Content-Type": historyPath[3] ? "application/json" : "application/gzip",
              ...(historyPath[3]
                ? {}
                : {
                    "X-Replay-SHA256": record.replaySha256,
                    "Access-Control-Expose-Headers": "X-Replay-SHA256",
                  }),
              "Content-Disposition": `attachment; filename="${historyPath[3] ?? historyPath[1]}.${historyPath[3] ? "json" : "ndjson.gz"}"`,
            },
          });
        } catch {
          return reply({ error: "History unavailable" }, 503);
        }
      }
      if (
        env.REQUIRE_ENTITLEMENT !== "false" &&
        !account.capabilities.includes("avoidant:competitive")
      )
        return reply({ error: "Competitive entitlement required" }, 403);
      const quota = await env.MATCHES.get(env.MATCHES.idFromName(`quota:${account.id}`)).fetch(
        new Request("https://internal/quota", { method: "POST" }),
      );
      if (!quota.ok) return reply({ error: "Too many lobby requests" }, 429);
      if (url.pathname === "/matches") {
        const raw = await boundedBody(request, 2048);
        if (raw === null) return reply({ error: "Request too large" }, 413);
        const input = JSON.parse(raw);
        if (
          (input.matchMinutes !== undefined &&
            (!Number.isInteger(input.matchMinutes) ||
              input.matchMinutes < 1 ||
              input.matchMinutes > 1440)) ||
          (input.turnSeconds !== undefined &&
            input.turnSeconds !== null &&
            (!Number.isInteger(input.turnSeconds) ||
              input.turnSeconds < 1 ||
              input.turnSeconds > 86400)) ||
          !Object.hasOwn(catalog, input.map) ||
          !["collaborative", "territory"].includes(input.mode) ||
          typeof input.bordering !== "boolean" ||
          typeof input.turnBased !== "boolean" ||
          !(
            input.floodLimit === null ||
            (Number.isSafeInteger(input.floodLimit) &&
              input.floodLimit >= 0 &&
              input.floodLimit <= 4096)
          )
        )
          return reply({ error: "Invalid match options" }, 400);
        const id = crypto.randomUUID();
        response = await env.MATCHES.get(env.MATCHES.idFromName(id)).fetch(
          new Request("https://internal/create", {
            method: "POST",
            body: JSON.stringify({ ...input, actor: account.id, id }),
          }),
        );
        if (response.ok) {
          const invitation = await env.INVITATIONS?.register(id);
          response = reply({ id, code: invitation?.code, invitationUrl: invitation?.url }, 201);
        }
      } else {
        const raw = await boundedBody(request, 2048);
        if (raw === null) return reply({ error: "Request too large" }, 413);
        const input = raw ? JSON.parse(raw) : {};
        if (
          input.name !== undefined &&
          (typeof input.name !== "string" ||
            !input.name.trim() ||
            [...input.name.trim()].length > 32 ||
            /[\u0000-\u001f\u007f-\u009f]/u.test(input.name))
        )
          return reply({ error: "Invalid player name" }, 400);
        response = await env.MATCHES.get(env.MATCHES.idFromName(match![1])).fetch(
          new Request("https://internal/connect", {
            method: "POST",
            body: JSON.stringify({
              actor: account.id,
              authExpires: account.expiresAt,
              name: input.name,
            }),
          }),
        );
        if (response.ok) {
          const data = (await response.json()) as { ticket: string };
          const invitation = await env.INVITATIONS?.register(match![1]);
          response = reply({ ...data, id: match![1], invitationUrl: invitation?.url });
        }
      }
    } catch {
      response = reply({ error: "Request could not be completed" }, 400);
    }
    const headers = new Headers(response.headers);
    for (const [key, value] of Object.entries(cors)) headers.set(key, value);
    return new Response(response.body, { status: response.status, headers });
  },
} satisfies ExportedHandler<Env>;

export class MatchRoom extends DurableObject<Env> {
  private saved?: Saved;
  private environment: Env;
  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    this.environment = env;
    ctx.blockConcurrencyWhile(async () => {
      this.saved = await ctx.storage.get<Saved>("match");
    });
  }
  private async persist(result: Result, command?: unknown) {
    const changed =
      command !== undefined && result.snapshot.revision !== this.saved?.snapshot.revision;
    const engines = [...(this.saved?.engines ?? [])];
    if (changed && !engines.includes(engineHash)) {
      // Preserve the exact executable rules, including across later deployments.
      if (!(await this.environment.REPLAYS.head(`engines/${engineHash}.json`)))
        await this.environment.REPLAYS.put(`engines/${engineHash}.json`, engineBundle);
      engines.push(engineHash);
    }
    const saved = {
      ...this.saved!,
      state: result.state,
      snapshot: result.snapshot,
      engines,
      eventCount: (this.saved?.eventCount ?? 0) + (changed ? 1 : 0),
      // Reconnects and rejected moves must not postpone finished-room cleanup.
      cleanupAt: result.snapshot.finished
        ? (this.saved?.cleanupAt ?? Date.now() + 60_000)
        : undefined,
    };
    // One atomic multi-key write commits the authoritative state and its journal entry.
    await this.ctx.storage.put({
      match: saved,
      ...(changed
        ? {
            [`event:${String(saved.eventCount).padStart(8, "0")}`]: { engine: engineHash, command },
          }
        : {}),
    });
    this.saved = saved;
    const s = result.snapshot;
    const alarm = s.finished
      ? saved.cleanupAt!
      : s.started
        ? Math.min(s.deadline, s.turn === null || !s.turnDeadline ? s.deadline : s.turnDeadline)
        : this.saved.created + 10 * 60_000;
    await this.ctx.storage.setAlarm(alarm);
    if (s.finished) {
      try {
        await this.archive();
      } catch {
        /* The cleanup alarm retries before deleting state. */
      }
    }
  }
  private async archive() {
    const saved = this.saved;
    if (!saved || saved.archived || !saved.snapshot.finished) return;
    if (!saved.id) {
      saved.id = crypto.randomUUID();
      await this.ctx.storage.put("match", saved);
    }
    const snapshot = saved.snapshot;
    if (saved.initial && !saved.replayKey) {
      const storage = this.ctx.storage;
      const encoder = new TextEncoder();
      async function* lines() {
        yield JSON.stringify({ type: "initial", ...saved!.initial }) + "\n";
        let after: string | undefined;
        while (true) {
          const page = await storage.list({
            prefix: "event:",
            limit: 1000,
            ...(after ? { startAfter: after } : {}),
          });
          for (const [key, event] of page) {
            yield JSON.stringify({ type: "command", ...(event as object) }) + "\n";
            after = key;
          }
          if (page.size < 1000) break;
        }
        yield JSON.stringify({ type: "final", state: saved!.state }) + "\n";
      }
      const iterator = lines();
      const stream = new ReadableStream<BufferSource>({
        async pull(controller) {
          const line = await iterator.next();
          if (line.done) controller.close();
          else controller.enqueue(encoder.encode(line.value));
        },
      }).pipeThrough(new CompressionStream("gzip"));
      const compressed = await new Response(stream).arrayBuffer();
      const hash = Array.from(
        new Uint8Array(await crypto.subtle.digest("SHA-256", compressed)),
        (b) => b.toString(16).padStart(2, "0"),
      ).join("");
      const key = `avoidant/${saved.id}/${hash}.ndjson.gz`;
      await this.environment.REPLAYS.put(key, compressed, {
        httpMetadata: { contentType: "application/gzip" },
      });
      saved.replayKey = key;
      saved.replaySha256 = hash;
      await this.ctx.storage.put("match", saved);
    }
    await saveHistory(this.environment, {
      id: saved.id!,
      map: saved.map,
      options: snapshot.options,
      startedAt: snapshot.startedAt,
      finishedAt: snapshot.finishedAt ?? saved.cleanupAt! - 60_000,
      players: snapshot.players,
      // Legacy live rooms cannot recover a lost starting board or move log.
      initial: saved.initial ?? { format: 1, unavailable: true },
      replayKey: saved.replayKey ?? null,
      replaySha256: saved.replaySha256 ?? null,
      engines: saved.engines ?? [],
    });
    saved.archived = true;
    await this.ctx.storage.put("match", saved);
  }
  private broadcast() {
    const payload = JSON.stringify({
      type: "snapshot",
      snapshot: this.saved!.snapshot,
      map: this.saved!.map,
      neighbors: catalog[this.saved!.map].neighbors,
    });
    for (const ws of this.ctx.getWebSockets()) {
      try {
        ws.send(payload);
      } catch {
        ws.close(1011, "Send failed");
      }
    }
  }
  async fetch(request: Request): Promise<Response> {
    // Serialize fetch and WS mutations across every awaited storage operation.
    return this.ctx.blockConcurrencyWhile(async () => {
      const url = new URL(request.url);
      if (url.pathname === "/quota") {
        const now = Date.now();
        const quota = (await this.ctx.storage.get<{ start: number; count: number }>("quota")) ?? {
          start: now,
          count: 0,
        };
        if (now - quota.start > 3600_000) {
          quota.start = now;
          quota.count = 0;
        }
        if (++quota.count > 60) return json({ error: "Rate limit" }, 429);
        await this.ctx.storage.put("quota", quota);
        await this.ctx.storage.setAlarm(quota.start + 3600_000);
        return json({ ok: true });
      }
      if (url.pathname === "/create") {
        if (this.saved) return json({ error: "Already created" }, 409);
        const input = (await request.json()) as {
          map: keyof typeof catalog;
          mode: "collaborative" | "territory";
          bordering: boolean;
          turnBased: boolean;
          floodLimit: number | null;
          matchMinutes?: number;
          turnSeconds?: number | null;
          actor: string;
          id: string;
        };
        const seed = Array.from(crypto.getRandomValues(new Uint8Array(32)), (b) =>
          b.toString(16).padStart(2, "0"),
        ).join("");
        const result = await rules("", {
          kind: "create",
          neighbors: catalog[input.map].neighbors,
          fraction: 0.15625,
          seed,
          actor: input.actor,
          options: {
            mode: input.mode,
            bordering: input.bordering,
            turnBased: input.turnBased,
            floodLimit: input.floodLimit,
            matchMinutes: input.matchMinutes ?? 20,
            turnSeconds: input.turnSeconds === undefined ? 30 : input.turnSeconds,
          },
        });
        this.saved = {
          state: result.state,
          snapshot: result.snapshot,
          map: input.map,
          created: Date.now(),
          id: input.id,
          initial: {
            format: 1,
            state: result.state,
            map: catalog[input.map].options,
            engine: engineHash,
          },
          engines: [engineHash],
          eventCount: 0,
        };
        if (!(await this.environment.REPLAYS.head(`engines/${engineHash}.json`)))
          await this.environment.REPLAYS.put(`engines/${engineHash}.json`, engineBundle);
        await this.persist(result);
        return json({ ok: true });
      }
      if (!this.saved) return json({ error: "Match not found" }, 404);
      if (url.pathname === "/connect") {
        const { actor, authExpires, name } = (await request.json()) as {
          actor: string;
          authExpires: number;
          name?: string;
        };
        const command = {
          kind: "join",
          actor,
          name,
          now: Date.now(),
        };
        const result = await rules(this.saved.state, command);
        await this.persist(result, command);
        if (result.error) return json({ error: result.error }, 409);
        const ticket = crypto.randomUUID();
        const tickets = await this.ctx.storage.list({ prefix: "ticket:" });
        // Bounded transient tickets, including reconnects that never upgraded.
        for (const [key, value] of tickets)
          if ((value as { expires: number }).expires < Date.now())
            await this.ctx.storage.delete(key);
        if (tickets.size >= 32) return json({ error: "Too many pending connections" }, 429);
        await this.ctx.storage.put(`ticket:${ticket}`, {
          actor,
          authExpires,
          expires: Date.now() + 30_000,
        });
        this.broadcast();
        return json({ ticket });
      }
      if (
        url.pathname.endsWith("/socket") &&
        request.headers.get("Upgrade")?.toLowerCase() === "websocket"
      ) {
        const ticket = url.searchParams.get("ticket") ?? "";
        if (!uuid.test(ticket)) return json({ error: "Invalid ticket" }, 401);
        const entry = await this.ctx.storage.get<{
          actor: string;
          expires: number;
          authExpires: number;
        }>(`ticket:${ticket}`);
        await this.ctx.storage.delete(`ticket:${ticket}`);
        if (!entry || entry.expires < Date.now()) return json({ error: "Ticket expired" }, 401);
        if (this.ctx.getWebSockets().length >= 16)
          return json({ error: "Room connection limit" }, 429);
        const pair = new WebSocketPair();
        this.ctx.acceptWebSocket(pair[1]);
        pair[1].serializeAttachment({
          actor: entry.actor,
          expires: Math.min(entry.authExpires, Date.now() + 15 * 60_000),
          window: Date.now(),
          messages: 0,
        } satisfies Attachment);
        pair[1].send(
          JSON.stringify({
            type: "snapshot",
            snapshot: this.saved.snapshot,
            map: this.saved.map,
            neighbors: catalog[this.saved.map].neighbors,
          }),
        );
        return new Response(null, { status: 101, webSocket: pair[0] });
      }
      return json({ error: "Not found" }, 404);
    });
  }
  async webSocketMessage(ws: WebSocket, message: string | ArrayBuffer) {
    await this.ctx.blockConcurrencyWhile(async () => {
      const a = ws.deserializeAttachment() as Attachment;
      if (!this.saved || Date.now() > a.expires) {
        ws.close(4001, "Reconnect to authenticate");
        return;
      }
      if (typeof message !== "string" || message.length > 512) {
        ws.close(1009, "Invalid payload");
        return;
      }
      if (Date.now() - a.window > 1000) {
        a.window = Date.now();
        a.messages = 0;
      }
      if (++a.messages > 10) {
        ws.close(1008, "Rate limit");
        return;
      }
      ws.serializeAttachment(a);
      try {
        const input = JSON.parse(message);
        let command: unknown;
        if (input.kind === "start") command = { kind: "start", actor: a.actor, now: Date.now() };
        else if (
          input.kind === "move" &&
          Number.isSafeInteger(input.sequence) &&
          input.sequence > 0
        )
          command = {
            kind: "move",
            actor: a.actor,
            sequence: input.sequence,
            action: input.action,
            now: Date.now(),
          };
        else throw new Error("Invalid action");
        const result = await rules(this.saved.state, command);
        await this.persist(result, command);
        this.broadcast();
        if (result.error) ws.send(JSON.stringify({ type: "error", error: result.error }));
      } catch {
        ws.send(JSON.stringify({ type: "error", error: "Invalid action" }));
      }
    });
  }
  webSocketClose(ws: WebSocket, code: number, reason: string) {
    ws.close(code, reason);
  }
  async alarm() {
    await this.ctx.blockConcurrencyWhile(async () => {
      if (!this.saved || this.saved.snapshot.finished || !this.saved.snapshot.started) {
        if (this.saved?.snapshot.finished) {
          try {
            await this.archive();
          } catch {
            for (const ws of this.ctx.getWebSockets())
              ws.close(1011, "Match complete; archive pending");
            await this.ctx.storage.setAlarm(Date.now() + 15 * 60_000);
            return;
          }
        }
        for (const ws of this.ctx.getWebSockets())
          ws.close(1000, this.saved?.snapshot.finished ? "Match complete" : "Match expired");
        const retiring = this.saved?.id ?? (await this.ctx.storage.get<string>("retiring"));
        if (retiring) await this.ctx.storage.put("retiring", retiring);
        this.saved = undefined;
        await this.ctx.storage.delete("match");
        try {
          if (retiring) await this.environment.INVITATIONS?.retire(retiring);
          await this.ctx.storage.deleteAll();
        } catch {
          // Keep the code reserved until cleanup notification succeeds.
          await this.ctx.storage.setAlarm(Date.now() + 60_000);
        }
        return;
      }
      const command = { kind: "tick", now: Date.now() };
      await this.persist(await rules(this.saved.state, command), command);
      this.broadcast();
    });
  }
}
