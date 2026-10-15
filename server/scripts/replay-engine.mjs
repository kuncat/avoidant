import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const root = new URL("../rules/", import.meta.url);
const bundle = JSON.stringify({
  wasm: readFileSync(new URL("avoidant_competitive_bg.wasm", root)).toString("base64"),
  glue: readFileSync(new URL("avoidant_competitive.js", root), "utf8"),
});
const hash = createHash("sha256").update(bundle).digest("hex");
writeFileSync(
  new URL("replay-engine.ts", root),
  `export const engineHash = ${JSON.stringify(hash)};\nexport const engineBundle = ${JSON.stringify(bundle)};\n`,
);
