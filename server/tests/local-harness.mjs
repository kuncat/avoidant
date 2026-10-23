// Only this local harness mocks identity. Never included in the Worker bundle.
import { database } from "./local-database.mjs";
import { Miniflare } from "miniflare";
import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { spawn } from "node:child_process";
import { createServer, isIP } from "node:net";
export async function startLocalMultiplayer({
  lanHost,
  webPort = 5174,
  serverPort = 8799,
  games = false,
} = {}) {
  if (lanHost && (isIP(lanHost) !== 4 || lanHost === "0.0.0.0")) {
    throw new Error("--host must be this machine's reachable IPv4 address (e.g. 192.168.1.20).");
  }
  const bindHost = lanHost ? "0.0.0.0" : "127.0.0.1";
  const browserOrigin = `http://${lanHost ?? "127.0.0.1"}:${webPort}`;

  const root = fileURLToPath(new URL("../../", import.meta.url));
  let pg;
  let vite,
    mf,
    logs = "";
  const close = async () => {
    if (vite && vite.exitCode === null && vite.signalCode === null) {
      const stopped = new Promise((resolve) => vite.once("exit", resolve));
      vite.kill("SIGTERM");
      await stopped;
    }
    await mf?.dispose();
    await pg?.db.close();
  };
  try {
    // Refuse occupied ports before starting child processes.
    for (const port of [webPort, serverPort]) {
      await new Promise((resolve, reject) => {
        const probe = createServer();
        probe.once("error", reject);
        probe.listen(port, bindHost, () => probe.close(resolve));
      });
    }
    pg = await database();
    mf = new Miniflare({
      port: serverPort,
      host: bindHost,
      modules: [
        { type: "ESModule", path: resolve(root, "server/dist/index.js") },
        ...readdirSync(resolve(root, "server/dist"))
          .filter((f) => f.endsWith(".wasm"))
          .map((f) => ({ type: "CompiledWasm", path: resolve(root, "server/dist", f) })),
      ],
      compatibilityDate: "2026-06-01",
      durableObjects: { MATCHES: { className: "MatchRoom", useSQLite: true } },
      r2Buckets: ["REPLAYS"],
      bindings: {
        DATABASE_URL: "postgresql://test:test@db.test/kuncat",
        ALLOWED_ORIGINS: [...new Set([`http://127.0.0.1:${webPort}`, browserOrigin])].join(","),
        ACCOUNT_API: "https://accounts.test",
      },
      outboundService: async (request) => {
        if (new URL(request.url).pathname === "/sql") return pg.fetch(request);
        return Response.json({
          id: await pg.account(request.headers.get("Authorization").slice(7)),
          capabilities: ["games:host"],
          expiresAt: Date.now() + 900000,
        });
      },
    });
    await mf.ready;
    vite = spawn(
      process.execPath,
      [
        "node_modules/vite/bin/vite.js",
        "--host",
        bindHost,
        "--port",
        String(webPort),
        "--strictPort",
      ],
      {
        cwd: games ? resolve(root, "../kuncat.com/games") : root,
        env: {
          ...process.env,
          VITE_LOCAL_COMPETITIVE_PORT: String(serverPort),
          ...(games
            ? {
                PUBLIC_KUNCAT_CLIENT_ID: "local-test",
                PUBLIC_AVOIDANT_SERVER_URL: `http://127.0.0.1:${serverPort}`,
              }
            : {}),
        },
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    vite.stdout.on("data", (b) => (logs = (logs + b).slice(-20000)));
    vite.stderr.on("data", (b) => (logs = (logs + b).slice(-20000)));
    let spawnError;
    vite.on("error", (error) => {
      spawnError = error;
    });
    let ready = false;
    for (let i = 0; i < 100; i++) {
      if (spawnError) throw spawnError;
      if (vite.exitCode !== null) throw new Error(`Vite exited: ${logs}`);
      try {
        await fetch(`http://127.0.0.1:${webPort}`);
        ready = true;
        break;
      } catch {
        await new Promise((r) => setTimeout(r, 100));
      }
    }
    if (!ready) throw new Error(`Vite did not start: ${logs}`);
    return { close, url: `${browserOrigin}/`, logs: () => logs };
  } catch (error) {
    await close();
    throw error;
  }
}
