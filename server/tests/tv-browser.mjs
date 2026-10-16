import { startLocalMultiplayer } from "./local-harness.mjs";
import { localActorId } from "../../src/lib/multiplayer/local-identity.ts";
import { chromium } from "playwright";
import assert from "node:assert/strict";
let h, browser;
try {
  h = await startLocalMultiplayer({ webPort: 5176, serverPort: 8797, games: true });
  browser = await chromium.launch({ headless: true, args: ["--no-sandbox"] });
  const errors = [];
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const tv = await context.newPage();
  tv.on("pageerror", (e) => errors.push(e.message));
  await tv.goto(h.url);
  await tv.getByRole("switch", { name: "TV Host Mode" }).check();
  await tv.getByRole("link", { name: /Avoidant/ }).click();
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
  await tv.locator("canvas").waitFor();
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
  await tv.screenshot({ path: "/tmp/avoidant-tv-after.png" });
  await tv.reload();
  await tv.locator("canvas").waitFor();
  assert.deepEqual(errors, []);
  console.log(
    "PASS: launcher TV mode, phone host + join, spectator rendering, arrow controls, and TV reload",
  );
} finally {
  await browser?.close();
  await h?.close();
}
