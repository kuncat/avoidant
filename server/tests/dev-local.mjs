import { startLocalMultiplayer } from "./local-harness.mjs";
import { networkInterfaces } from "node:os";
const args = process.argv.slice(2);
if (args.includes("--help")) {
  console.log(
    "Usage: npm run dev:multiplayer -- [--host [LAN_IP]]\n--host binds both servers on all interfaces; an omitted IP is auto-detected.",
  );
  process.exit(0);
}
let lanHost;
if (args.length) {
  if (args[0] === "--host" && args.length <= 2) lanHost = args[1] ?? "0.0.0.0";
  else if (args.length === 1 && args[0].startsWith("--host="))
    lanHost = args[0].slice(7) || "0.0.0.0";
  else throw new Error("Expected --host [LAN_IP]. Use --help for usage.");
  if (lanHost === "0.0.0.0") {
    lanHost = Object.values(networkInterfaces())
      .flat()
      .find((address) => address?.family === "IPv4" && !address.internal)?.address;
    if (!lanHost)
      throw new Error(
        "No LAN IPv4 address found. Pass --host with this machine's reachable IPv4 address.",
      );
  }
}
const harness = await startLocalMultiplayer({ lanHost });
console.log(`Local multiplayer (test accounts; ${lanHost ? "LAN access enabled — use only on a trusted network" : "loopback only"}):
Player 1: ${harness.url}?actor=a
Player 2: ${harness.url}?actor=b
Choose New Game and a multiplayer mode in Player 1. Open its invitation in Player 2, then Start match.
Additional territory players: change actor to c, d, etc. Ctrl+C stops both servers.
State is temporary. Real registration/login is not exercised here.`);
let closing = false;
async function stop() {
  if (closing) return;
  closing = true;
  try {
    await harness.close();
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  }
}
process.once("SIGINT", stop);
process.once("SIGTERM", stop);
