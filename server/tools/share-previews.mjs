// Regenerate static share cards using the real map generator and Karrom canvas renderer.
// Run after npm run build:wasm. Arguments: output directory, optional board.ts path.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { chromium } from "playwright";
import { initSync, generateMapData } from "../../src/lib/wasm/avoidant_wasm.js";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const out = resolve(
  process.argv[2] ?? resolve(root, "../infra/platform/invitations/public/previews/v1"),
);
const website = resolve(root, "../kuncat.com/games/src");
const board =
  process.argv[3] ??
  ["karrom", "carrom"].map((name) => `${website}/${name}/board.ts`).find(existsSync);
const { build } = createRequire(new URL("../package.json", import.meta.url))("esbuild");
const bundled = await build({
  entryPoints: [board],
  bundle: true,
  write: false,
  format: "iife",
  globalName: "KarromRenderer",
});
initSync({ module: readFileSync(resolve(root, "src/lib/wasm/avoidant_wasm_bg.wasm")) });
const presets = JSON.parse(readFileSync(resolve(root, "src/lib/multiplayer/maps.json"), "utf8"));
const labels = {
  cube: "Cube",
  dodecahedron: "Dodecahedron",
  flat: "Flat",
  geodesicIcosphere: "Geodesic sphere",
  octahedron: "Octahedron",
  spheroid: "Spheroid",
  tetrahedron: "Tetrahedron",
};
mkdirSync(out, { recursive: true });
const manifest = {};
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1200, height: 630 },
    deviceScaleFactor: 1,
  });
  await page.setContent(
    '<style>body{margin:0;background:#0b1724}canvas{display:block}</style><canvas id="card" width="1200" height="630"></canvas><canvas id="board" style="width:500px;height:500px"></canvas>',
  );
  await page.addScriptTag({ content: bundled.outputFiles[0].text });
  for (const [id, options] of [...Object.entries(presets), ["karrom", null], ["table", null]]) {
    const map = options ? generateMapData(options) : null;
    const label = options
      ? `${labels[options.shape.kind] ?? options.shape.kind} · ${id.split("-").at(-1)}`
      : "";
    await page.evaluate(
      ({ map, label, id, options }) => {
        const canvas = document.getElementById("card"),
          c = canvas.getContext("2d");
        const gradient = c.createLinearGradient(0, 0, 1200, 630);
        gradient.addColorStop(0, "#091522");
        gradient.addColorStop(1, "#1e3b49");
        c.fillStyle = gradient;
        c.fillRect(0, 0, 1200, 630);
        c.fillStyle = "#72ddc9";
        c.font = "bold 19px Arial";
        c.fillText("KUNCAT GAMES", 64, 92);
        c.fillStyle = "#f4f7f8";
        c.font = "bold 64px Arial";
        c.fillText(map ? "Avoidant" : id === "karrom" ? "Karrom" : "Game night", 60, 220);
        c.font = "24px Arial";
        c.fillStyle = "#c4d4de";
        c.fillText(map ? label : "A game with friends.", 64, 277);
        if (map) {
          c.font = "20px Arial";
          c.fillText(`${options.numCells} cells`, 64, 317);
        }
        c.fillStyle = "#72ddc9";
        c.font = "bold 22px Arial";
        c.fillText("JOIN THE GAME  →", 64, 464);
        c.fillStyle = "#95acba";
        c.font = "18px Arial";
        c.fillText("kuga.run", 64, 555);
        if (map) {
          // Draw the actual seeded polygon geometry; no hidden voids or live state.
          const rotate = ([x, y, z]) => {
            let a = 0.55,
              b = -0.45;
            let X = x * Math.cos(a) + z * Math.sin(a),
              Z = -x * Math.sin(a) + z * Math.cos(a);
            return [X, y * Math.cos(b) - Z * Math.sin(b), y * Math.sin(b) + Z * Math.cos(b)];
          };
          const cells = map.cells
            .map((cell, i) => {
              const vs = cell.vertices.map(rotate);
              return { vs, i, z: vs.reduce((s, v) => s + v[2], 0) / vs.length };
            })
            .sort((a, b) => a.z - b.z);
          const xs = cells.flatMap((p) => p.vs.map((v) => v[0])),
            ys = cells.flatMap((p) => p.vs.map((v) => v[1]));
          const minX = Math.min(...xs),
            maxX = Math.max(...xs),
            minY = Math.min(...ys),
            maxY = Math.max(...ys);
          const scale = Math.min(570 / (maxX - minX), 470 / (maxY - minY));
          for (const p of cells) {
            c.beginPath();
            p.vs.forEach(([x, y], i) => {
              const X = 850 + (x - (minX + maxX) / 2) * scale,
                Y = 315 - (y - (minY + maxY) / 2) * scale;
              i ? c.lineTo(X, Y) : c.moveTo(X, Y);
            });
            c.closePath();
            c.fillStyle = `hsl(${174 + (p.i % 5) * 7} 35% ${32 + ((p.i * 17) % 27)}%)`;
            c.fill();
            c.strokeStyle = "#0e2734";
            c.lineWidth = 1.2;
            c.stroke();
          }
        } else if (id === "karrom") {
          const boardCanvas = document.getElementById("board");
          const board = new KarromRenderer.Board(boardCanvas);
          const coins = [{ x: 500, y: 500, color: 2 }];
          for (let ring = 1; ring <= 2; ring++)
            for (let i = 0; i < ring * 6; i++) {
              const a = (i * Math.PI * 2) / (ring * 6);
              coins.push({
                x: 500 + ring * 33 * Math.cos(a),
                y: 500 + ring * 33 * Math.sin(a),
                color: i % 2,
              });
            }
          board.update({
            protocol: 1,
            revision: 0,
            serverTime: 0,
            readyAt: 0,
            aim: null,
            state: { version: 1, coins, phase: "playing", options: { players: 2 }, turn: 0 },
          });
          board.draw();
          c.drawImage(boardCanvas, 590, 65, 500, 500);
          board.destroy();
        }
      },
      { map, label, id, options },
    );
    await page.locator("#card").screenshot({ path: resolve(out, id + ".png") });
    if (options) manifest[id] = { label, cells: options.numCells, image: id + ".png" };
  }
  writeFileSync(
    resolve(out, "../../../src/preview-maps.json"),
    JSON.stringify(manifest, null, 2) + "\n",
  );
  console.log(`Generated ${Object.keys(manifest).length} map cards and two game cards in ${out}`);
} finally {
  await browser.close();
}
