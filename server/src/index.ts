import { DurableObject } from "cloudflare:workers";
import catalog from "./catalog.json";
import { rules, type Result, type Snapshot } from "./rules";

interface Env {
  MATCHES: DurableObjectNamespace<MatchRoom>;
  ALLOWED_ORIGINS: string;
  ACCOUNT_API: string;
  REQUIRE_ENTITLEMENT: string;
}
interface Saved {
  state: string;
  snapshot: Snapshot;
  map: keyof typeof catalog;
  created: number;
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
      const match = /^\/matches\/([^/]+)(?:\/(connect|socket))?$/.exec(url.pathname);
      if (match && uuid.test(match[1]) && match[2] === "socket" && request.method === "GET") {
        // The room consumes a short-lived single-use ticket; never put bearer tokens in URLs.
        return await env.MATCHES.get(env.MATCHES.idFromName(match[1])).fetch(request);
      }
      if (
        request.method !== "POST" ||
        (url.pathname !== "/matches" && !(match && uuid.test(match[1]) && match[2] === "connect"))
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
          !["collaborative", "duel", "territory"].includes(input.mode) ||
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
            body: JSON.stringify({ ...input, actor: account.id }),
          }),
        );
        if (response.ok) response = reply({ id }, 201);
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
  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    ctx.blockConcurrencyWhile(async () => {
      this.saved = await ctx.storage.get<Saved>("match");
    });
  }
  private async persist(result: Result) {
    const saved = {
      ...this.saved!,
      state: result.state,
      snapshot: result.snapshot,
      // Reconnects and rejected moves must not postpone finished-room cleanup.
      cleanupAt: result.snapshot.finished
        ? (this.saved?.cleanupAt ?? Date.now() + 60_000)
        : undefined,
    };
    await this.ctx.storage.put("match", saved);
    this.saved = saved;
    const s = result.snapshot;
    const alarm = s.finished
      ? saved.cleanupAt!
      : s.started
        ? Math.min(s.deadline, s.turn === null || !s.turnDeadline ? s.deadline : s.turnDeadline)
        : this.saved.created + 10 * 60_000;
    await this.ctx.storage.setAlarm(alarm);
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
          mode: "duel" | "territory";
          bordering: boolean;
          turnBased: boolean;
          floodLimit: number | null;
          matchMinutes?: number;
          turnSeconds?: number | null;
          actor: string;
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
        };
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
        const result = await rules(this.saved.state, { kind: "join", actor, name });
        if (result.error) return json({ error: result.error }, 409);
        await this.persist(result);
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
        await this.persist(result);
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
        for (const ws of this.ctx.getWebSockets()) ws.close(1000, "Match expired");
        this.saved = undefined;
        await this.ctx.storage.deleteAll();
        return;
      }
      await this.persist(await rules(this.saved.state, { kind: "tick", now: Date.now() }));
      this.broadcast();
    });
  }
}
