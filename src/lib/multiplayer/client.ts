import { m } from "$lib/paraglide/messages";
import type { CompetitiveConfig, MatchSnapshot } from "./types";
/** One pending action at a time; retries reuse the sequence after reconnection. */
export class MatchClient {
  private socket?: WebSocket;
  private pending?: { kind: "move"; sequence: number; action: unknown };
  private stopped = false;
  private retry?: ReturnType<typeof setTimeout>;
  private retryCount = 0;
  constructor(
    private config: CompetitiveConfig,
    readonly id: string,
    private actor: string,
    private receive: (s: MatchSnapshot, map: string, neighbors: number[][]) => void,
    private report: (s: string) => void,
  ) {}
  async connect() {
    clearTimeout(this.retry);
    if (this.socket) {
      this.socket.onclose = null;
      this.socket.close();
    }
    try {
      const response = await fetch(`${this.config.serverUrl}/matches/${this.id}/connect`, {
        method: "POST",
        headers: { Authorization: `Bearer ${await this.config.account.token()}` },
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error ?? m.error_match_join());
      if (this.stopped) return;
      const url = new URL(`${this.config.serverUrl}/matches/${this.id}/socket`);
      url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
      url.searchParams.set("ticket", data.ticket);
      this.socket = new WebSocket(url);
      this.socket.onmessage = (event) => {
        const message = JSON.parse(event.data);
        if (message.type === "error") {
          this.pending = undefined;
          this.report(message.error);
          return;
        }
        if (message.type !== "snapshot") return;
        const s: MatchSnapshot = message.snapshot;
        if (s.version !== 1) {
          this.report(m.error_match_version());
          this.close();
          return;
        }
        const player = s.players.find((p) => p.id === this.actor);
        if (this.pending && player && player.sequence >= this.pending.sequence)
          this.pending = undefined;
        this.retryCount = 0;
        this.receive(s, message.map, message.neighbors);
        if (this.pending) this.socket?.send(JSON.stringify(this.pending));
      };
      this.socket.onclose = (event) => {
        if (this.stopped) return;
        if (event.code === 1000) {
          this.report(m.status_match_expired());
          return;
        }
        this.report(m.status_reconnecting());
        this.retry = setTimeout(
          () => void this.connect(),
          Math.min(1000 * 2 ** this.retryCount++, 15000),
        );
      };
      this.socket.onerror = () => this.report(m.error_connection_interrupted());
    } catch (error) {
      this.report(error instanceof Error ? error.message : m.error_connection_failed());
    }
  }
  start() {
    this.send({ kind: "start" });
  }
  move(sequence: number, action: unknown) {
    if (this.pending) return;
    if (this.socket?.readyState !== WebSocket.OPEN) {
      this.report(m.error_reconnect_to_play());
      return;
    }
    this.pending = { kind: "move", sequence, action };
    this.send(this.pending);
  }
  private send(value: unknown) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify(value));
  }
  close() {
    this.stopped = true;
    clearTimeout(this.retry);
    this.socket?.close();
  }
}
