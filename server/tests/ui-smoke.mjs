// Local-only harness; production has no authentication bypass.
import { startLocalMultiplayer } from "./local-harness.mjs";
import { chromium } from "playwright";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import assert from "node:assert/strict";
const scratch = mkdtempSync(`${tmpdir()}/avoidant-ui-`);
let browser, harness, page;
try {
  harness = await startLocalMultiplayer({
    lanHost: process.env.TEST_LAN_HOST,
    webPort: Number(process.env.TEST_WEB_PORT ?? 5174),
    serverPort: Number(process.env.TEST_SERVER_PORT ?? 8799),
  });
  browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    args: ["--no-sandbox"],
  });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  context.setDefaultTimeout(10000);
  const a = await context.newPage();
  page = a;
  const b = await context.newPage();
  const errors = [];
  for (const p of [a, b]) {
    p.on("console", (message) => {
      if (message.type() === "error" && /WebGL|Shader Error/.test(message.text()))
        errors.push(message.text());
    });
    p.on("pageerror", (e) => {
      errors.push(e.message);
      console.error("Browser error:", e.message);
    });
  }
  await a.goto(`${harness.url}?actor=a`);
  await a.getByRole("button", { name: "New Game", exact: true }).click();
  const mode = process.env.TEST_MODE ?? "territory";
  const modeLabels = {
    collaborative: "Collaborative Explore together",
    duel: "Duel Take turns to score",
    territory: "Territory Claim the board",
  };
  await a.getByRole("button", { name: modeLabels[mode], exact: true }).click();
  assert.equal(await a.getByLabel("Player Name", { exact: true }).inputValue(), "a");
  await a.getByLabel("Player Name", { exact: true }).fill("Asha");
  const duration = a.getByLabel("Match duration", { exact: true });
  await a.getByLabel("Duration unit").selectOption("hours");
  assert.equal(await duration.getAttribute("max"), "24");
  await duration.fill("24");
  await a.getByLabel("Duration unit").selectOption("minutes");
  assert.equal(await duration.getAttribute("max"), "1440");
  assert.equal(await duration.inputValue(), "1440");
  const mapNames = {
    flat: "Flat",
    icosahedron: "Icosahedron",
    cube: "Cube",
    tetrahedron: "Tetrahedron",
    octahedron: "Octahedron",
    dodecahedron: "Dodecahedron",
    spheroid: "Spheroid",
    geodesicIcosphere: "Geodesic icosphere",
  };
  const maps = a.getByRole("group", { name: "Map", exact: true });
  assert.equal(await maps.getByRole("button").count(), 8);
  await maps.getByRole("button", { name: "Cube", exact: true }).click();
  await maps
    .getByRole("button", {
      name: mapNames[process.env.TEST_MAP_SHAPE ?? "icosahedron"],
      exact: true,
    })
    .click();
  await a
    .getByRole("group", { name: "Map Options", exact: true })
    .filter({ visible: true })
    .last()
    .waitFor();
  if (mode === "territory") {
    await a.getByLabel("Claims must border existing territory").uncheck();
    await a.getByLabel("Turn-based", { exact: true }).check();
  }
  await a.getByLabel("Additional flood cells").click();
  await a.getByLabel("Additional flood cells").fill("3");
  await a.getByRole("button", { name: "Infinite flood", exact: true }).click();
  assert.equal(
    await a
      .getByRole("button", { name: "Infinite flood", exact: true })
      .getAttribute("aria-pressed"),
    "true",
  );
  await a.getByLabel("Additional flood cells").click();
  assert.equal(await a.getByLabel("Additional flood cells").inputValue(), "3");
  await a.getByLabel("Additional flood cells").fill("0");
  assert.equal(
    await a.getByLabel("Additional flood cells").evaluate((el) => el.checkValidity()),
    false,
  );
  await a.getByLabel("Additional flood cells").fill("3");
  assert.ok(
    await a
      .locator("#flood-limit")
      .evaluate((el) =>
        Boolean(
          el.compareDocumentPosition(document.querySelector("#map-label")) &
          Node.DOCUMENT_POSITION_FOLLOWING,
        ),
      ),
  );

  await a.waitForTimeout(650); // Let the menu expansion and theme transitions settle.
  const rightEdges = await a
    .getByRole("group", { name: "Gameplay Options", exact: true })
    .evaluate((el) =>
      [...el.querySelectorAll('input[type="checkbox"], .join')].map(
        (control) => control.getBoundingClientRect().right,
      ),
    );
  assert.ok(Math.max(...rightEdges) - Math.min(...rightEdges) <= 1);
  await a.screenshot({ path: `${scratch}/setup.png`, animations: "disabled" });
  await a.getByRole("button", { name: "Start", exact: true }).click();
  await a.getByText("Waiting in lobby.", { exact: false }).waitFor();
  const invitation = await a.getByLabel("Invitation link", { exact: true }).inputValue();
  assert.ok(new URL(invitation).searchParams.has("match"));
  assert.equal(new URL(invitation).searchParams.has("actor"), false);
  // Select B's local identity, then follow exactly the shareable URL.
  await b.goto(`${harness.url}?actor=b`);
  await b.getByRole("button", { name: "New Game", exact: true }).waitFor();
  await b.goto(invitation);
  await b.getByText("Waiting in lobby.", { exact: false }).waitFor();
  await b.getByLabel("Player Name", { exact: true }).fill("Bela");
  await b.getByRole("button", { name: "Save name", exact: true }).click();
  await b.getByText("Name saved", { exact: true }).waitFor();
  const alignment = await b
    .getByRole("button", { name: "Save name", exact: true })
    .evaluate((button) => {
      const input = button.form.querySelector("input").getBoundingClientRect();
      return Math.abs(button.getBoundingClientRect().bottom - input.bottom);
    });
  assert.ok(alignment <= 1);
  await a.getByText("Bela", { exact: true }).waitFor();
  await a.getByRole("button", { name: "Start match", exact: true }).click();
  const startedText = mode === "collaborative" ? "Explore together" : "Asha's turn";
  await a.getByText(startedText, { exact: false }).waitFor();
  await b.getByText(startedText, { exact: false }).waitFor();
  assert.equal(await a.getByLabel("Invitation link", { exact: true }).count(), 0);
  assert.equal(await a.getByText("Match ID:", { exact: false }).count(), 0);
  assert.equal(await a.getByRole("heading", { name: "Avoidant", exact: true }).count(), 1);
  const score = a.getByRole("group", { name: "Score summary" }).getByRole("button");
  await score.click();
  await a.getByText("Asha (you)", { exact: true }).waitFor();
  await a.getByText("Bela", { exact: true }).waitFor();
  await a.getByRole("heading", { name: "Avoidant", exact: true }).click();
  assert.equal(await score.getAttribute("aria-expanded"), "true");
  await score.click();

  assert.equal(await score.getAttribute("aria-expanded"), "false");

  const initialTeam =
    mode === "collaborative" ? await b.getByText("Team score:").textContent() : null;
  await a.locator("canvas").waitFor();
  await a.screenshot({ path: `${scratch}/territory.png` });
  // Exercise the real 3D click path, not only the lobby controls.
  const canvas = a.locator("canvas");
  const box = await canvas.boundingBox();
  let moved = false;
  for (const fraction of [0.5, 0.45, 0.55]) {
    await canvas.click({ position: { x: box.width * fraction, y: box.height * 0.5 } });
    try {
      if (mode === "collaborative") {
        for (
          let i = 0;
          i < 15 && (await b.getByText("Team score:").textContent()) === initialTeam;
          i++
        )
          await b.waitForTimeout(100);
        assert.notEqual(await b.getByText("Team score:").textContent(), initialTeam);
      } else await b.getByText("Bela's turn", { exact: false }).waitFor({ timeout: 1500 });
      moved = true;
      break;
    } catch {
      /* Try another visible cell if the first was already owned. */
    }
  }
  assert.ok(moved, "A terrain click must reach the trusted authority");
  await a.waitForTimeout(500);
  await a.screenshot({ path: `${scratch}/territory.png` });
  if (mode !== "collaborative") {
    await b.getByRole("button", { name: "Pass", exact: true }).click();
    await a.getByText("Asha's turn", { exact: false }).waitFor();
    // Player A's preceding terrain action is subject to the 400 ms server cooldown.
    await a.waitForTimeout(450);
    await a.getByRole("button", { name: "Pass", exact: true }).click();
    await a.getByText("Match complete", { exact: false }).waitFor();
  }
  const locked = await context.newPage();
  await locked.goto(`${harness.url}?actor=logged-out`);
  await locked.getByRole("button", { name: "Log out", exact: true }).click();
  await locked.getByRole("button", { name: "New Game", exact: true }).click();
  for (const name of Object.values(modeLabels))
    assert.equal(await locked.getByRole("button", { name, exact: true }).isDisabled(), true);
  assert.equal(
    await locked
      .getByRole("button", { name: "Solo Explore at your pace", exact: true })
      .isDisabled(),
    false,
  );
  // Exercise the translated normal setup at a narrow viewport as well.
  const localized = await context.newPage();
  await localized.setViewportSize({ width: 390, height: 844 });
  await localized.goto(`${harness.url}?actor=a`);
  await localized.locator("#language-select").selectOption("bn");
  await localized.getByRole("button", { name: "নতুন গেম", exact: true }).click();
  await localized
    .getByRole("button", { name: "এলাকা দখল বোর্ডের এলাকা দখল করুন", exact: true })
    .click();
  await localized.getByLabel("দখল করা ঘর নিজের এলাকার সংলগ্ন হতে হবে").waitFor();
  await localized.getByRole("button", { name: "অসীম উন্মোচন", exact: true }).click();
  assert.equal(
    await localized
      .getByRole("button", { name: "অসীম উন্মোচন", exact: true })
      .getAttribute("aria-pressed"),
    "true",
  );
  await localized.waitForTimeout(400);
  assert.ok(await localized.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  const checkboxAlignment = await localized
    .getByLabel("পালা করে খেলা", { exact: true })
    .evaluate((el) => {
      const control = el.getBoundingClientRect();
      const text = el.previousElementSibling.getBoundingClientRect();
      return Math.abs((control.top + control.bottom) / 2 - (text.top + text.bottom) / 2);
    });
  assert.ok(checkboxAlignment <= 1, `Checkbox and label centers differ by ${checkboxAlignment}px`);
  await localized.screenshot({ path: `${scratch}/bengali-setup.png`, animations: "disabled" });
  await localized
    .getByRole("button", { name: "সহযোগিতামূলক একসঙ্গে অনুসন্ধান করুন", exact: true })
    .click();
  await localized.getByLabel("পালা করে খেলা", { exact: true }).waitFor({ state: "detached" });
  await localized.getByRole("button", { name: "শুরু করুন", exact: true }).click();
  await localized.getByText("খেলা শুরুর অপেক্ষায়।", { exact: false }).waitFor();
  assert.ok(await localized.getByLabel("আমন্ত্রণ লিঙ্ক", { exact: true }).inputValue());
  const solo = await context.newPage();
  await solo.goto(`${harness.url}?actor=a`);
  await solo.getByRole("button", { name: "নতুন গেম", exact: true }).click();
  await solo.getByRole("button", { name: "শুরু করুন", exact: true }).click();
  const soloScore = solo.getByRole("group", { name: "স্কোরের সারাংশ" }).getByRole("button");
  await soloScore.waitFor();
  await soloScore.focus();
  await soloScore.press("Enter");
  assert.equal(await soloScore.getAttribute("aria-expanded"), "true");
  await soloScore.press("Escape");
  assert.equal(await soloScore.getAttribute("aria-expanded"), "false");
  assert.deepEqual(errors, [], harness.logs());
  console.log(
    `${mode}: shared menu, invitation, authoritative terrain click, login gating and gameplay passed. Screenshot: ${scratch}/territory.png`,
  );
} catch (error) {
  console.error(harness?.logs());
  if (page) {
    console.error(await page.locator("body").innerText());
    await page.screenshot({ path: `${scratch}/failure.png` });
    console.error(`Screenshot: ${scratch}/failure.png`);
  }
  throw error;
} finally {
  await browser?.close();
  await harness?.close();
}
