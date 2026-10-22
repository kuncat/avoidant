import { startLocalMultiplayer } from "./local-harness.mjs";
import { localActorId } from "../../src/lib/multiplayer/local-identity.ts";
import { chromium } from "playwright";
import assert from "node:assert/strict";
import WebSocket from "ws";
import { readFile } from "node:fs/promises";
const packaged = !!process.env.PACKAGED_TV;
const fileOrigin = process.env.PACKAGED_TV === "file";
let h, browser;
try {
  h = await startLocalMultiplayer({ webPort: 5176, serverPort: 8797, games: true });
  browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE, args: ["--no-sandbox"] });
  const errors = [];
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  context.setDefaultTimeout(20000);
  const tv = await context.newPage();
  tv.on("pageerror", (e) => { errors.push(e.message); console.error(e.message); });
  if (packaged) {
    await context.route("https://avoidant.multiplayer.games.kuncat.com/**", async route => {
      const response = await route.fetch({ url: route.request().url().replace("https://avoidant.multiplayer.games.kuncat.com", "http://127.0.0.1:8797"), headers: { ...route.request().headers(), origin: "null" } });
      await route.fulfill({ response, headers: { ...response.headers(), "access-control-allow-origin": fileOrigin ? "null" : "https://games.kuncat.com" } });
    });
    await context.routeWebSocket("wss://avoidant.multiplayer.games.kuncat.com/**", ws => {
      const server = new WebSocket(ws.url().replace("wss://avoidant.multiplayer.games.kuncat.com", "ws://127.0.0.1:8797"), { origin: "null" });
      server.on("message", data => ws.send(data.toString()));
      server.on("close", () => ws.close());
      server.on("error", e => errors.push(e.message));
      ws.onClose(() => server.close());
      ws.onMessage(data => { if (server.readyState === WebSocket.OPEN) server.send(data); });
    });
    await context.route("https://games.kuncat.com/karrom/tv-bundle/**", async route => {
      const name = new URL(route.request().url()).pathname.split("tv-bundle/")[1];
      const type = name.endsWith(".js") ? "application/javascript" : name.endsWith(".css") ? "text/css" : name.endsWith(".svg") ? "image/svg+xml" : name.endsWith(".png") ? "image/png" : "text/html";
      await route.fulfill({ body: await readFile(new URL(`../../../karrom/dist/tv/${name}`, import.meta.url)), contentType: type });
    });
    await tv.goto(fileOrigin ? new URL("../../../karrom/dist/tv/index.html", import.meta.url).href : "https://games.kuncat.com/karrom/tv-bundle/index.html");
    await tv.getByRole("button", { name: "Karrom", exact: true }).click();
    await tv.locator("#games").click();
    await tv.getByRole("button", { name: "Avoidant", exact: true }).click();
  } else {
    await tv.goto(h.url);
    await tv.getByRole("switch", { name: "TV Host Mode" }).check();
    await tv.getByRole("link", { name: /Avoidant/ }).click();
  }
  await tv.getByRole("heading", { name: "Scan to host" }).waitFor();
  await tv.waitForFunction(() => !!sessionStorage.getItem("avoidant:tv-display"));
  const pair = await tv.evaluate(() => JSON.parse(sessionStorage.getItem("avoidant:tv-display")));
  const phone = async (actor) => {
    const c = await browser.newContext({ viewport: { width: 390, height: 844 } });
    await c.addInitScript(
      ({ actor, id }) => {
        window.__avoidantTestAccount = {
          login: async () => {},
          logout: async () => {},
          completeLogin: async () => {},
          token: async () => actor,
          account: async () => ({ id, username: actor, capabilities: [] }),
        };
      },
      { actor, id: localActorId(actor) },
    );
    const p = await c.newPage();
    p.on("pageerror", (e) => errors.push(e.message));
    return p;
  };
  const a = await phone("Host");
  let matchId;
  a.on("response", async (r) => {
    if (r.url().endsWith("/matches") && r.status() === 201) matchId = (await r.json()).id;
  });
  await a.goto(`${h.url}avoidant/#tv=${pair.id}.${pair.claim}`);
  await a.getByRole("button", { name: "Start", exact: true }).waitFor();
  assert.equal(await a.getByRole("button", { name: /^Solo/ }).count(), 0);
  await a.getByRole("button", { name: "Start", exact: true }).click();
  await a.getByRole("button", { name: "Start match", exact: true }).waitFor();
  assert.ok(matchId);
  const b = await phone("Guest");
  await b.goto(`${h.url}avoidant/?match=${matchId}`);
  await a.getByRole("button", { name: "Start match", exact: true }).click();
  await tv.getByRole("button", { name: "Map ↔ ↕" }).waitFor();
  await tv.locator("canvas").waitFor().catch(async e => { console.error(await tv.locator("body").innerText()); await tv.screenshot({path: "/tmp/packaged-tv-failure.png"}); throw e; });
  await tv.waitForTimeout(1500);
  await tv.screenshot({ path: "/tmp/avoidant-tv-before.png" });
  await tv.getByRole("button", { name: "Map ↔ ↕" }).click();
  const before = await tv.locator("canvas").screenshot();
  for (let i = 0; i < 8; i++) await tv.keyboard.press("ArrowRight");
  await tv.waitForTimeout(200);
  const after = await tv.locator("canvas").screenshot();
  assert.notDeepEqual(before, after, "TV arrow keys must move the camera");
  await tv.getByRole("button", { name: "Orbit", exact: true }).click();
  await tv.keyboard.press("ArrowUp");
  assert.equal(
    await tv.getByRole("button", { name: "Pan", exact: true }).getAttribute("aria-pressed"),
    "true",
  );
  await tv.getByRole("button", { name: "Hide QR", exact: true }).click();
  assert.equal(await tv.getByAltText("Join Avoidant on your phone").count(), 0);
  await tv.keyboard.press("q");
  await tv.getByAltText("Join Avoidant on your phone").waitFor();
  await tv.getByRole("button", { name: "Hide QR", exact: true }).waitFor();
  await tv.screenshot({ path: "/tmp/avoidant-tv-after.png" });
  await tv.reload();
  if (packaged) await tv.getByRole("button", { name: "Avoidant", exact: true }).click();
  await tv.locator("canvas").waitFor();
  if (packaged) {
    assert.equal(await tv.evaluate(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "BrowserBack", cancelable: true }))), false);
    assert.equal(await tv.getByRole("button", { name: "Map ↔ ↕" }).evaluate(el => el === document.activeElement), true);
    await tv.getByRole("button", { name: "Games", exact: true }).click();
    await tv.getByRole("heading", { name: "Kuncat Games", exact: true }).waitFor();
    assert.equal(await tv.evaluate(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "BrowserBack", cancelable: true }))), true);
  }
  assert.deepEqual(errors, []);
  console.log(
    "PASS: launcher TV mode, phone host + join, spectator rendering, arrow controls, and TV reload",
  );
} finally {
  await browser?.close();
  await h?.close();
}
