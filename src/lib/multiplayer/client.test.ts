import { afterEach, expect, it, vi } from "vitest";
import { MatchClient } from "./client";
import type { CompetitiveConfig } from "./types";

afterEach(() => vi.unstubAllGlobals());
it("silences stale cell clicks without presenting gameplay rejections as connection failures", async () => {
  const sockets: FakeSocket[] = [];
  class FakeSocket {
    onmessage?: (event: { data: string }) => void;
    constructor() {
      sockets.push(this);
    }
    close() {}
  }
  vi.stubGlobal("WebSocket", FakeSocket);
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(Response.json({ ticket: "test" })));
  const report = vi.fn();
  const config = {
    serverUrl: "https://game.example",
    account: { token: async () => "token" },
  } as CompetitiveConfig;
  const client = new MatchClient(config, "match", "player", vi.fn(), report, () => "Name");
  await client.connect();
  sockets[0].onmessage!({ data: JSON.stringify({ type: "error", error: "Cell unavailable" }) });
  expect(report).not.toHaveBeenCalled();
  sockets[0].onmessage!({ data: JSON.stringify({ type: "error", error: "Not your turn" }) });
  expect(report).toHaveBeenCalledWith("Not your turn");
  client.close();
});
