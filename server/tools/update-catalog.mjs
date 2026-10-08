import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
const root = new URL("../../", import.meta.url);
const catalog = JSON.parse(
  execFileSync("cargo", ["run", "-p", "avoidant", "--example", "competitive_catalog"], {
    cwd: root,
    encoding: "utf8",
  }),
);
writeFileSync(new URL("server/src/catalog.json", root), JSON.stringify(catalog) + "\n");
writeFileSync(
  new URL("src/lib/multiplayer/maps.json", root),
  JSON.stringify(
    Object.fromEntries(Object.entries(catalog).map(([id, map]) => [id, map.options])),
    null,
    2,
  ) + "\n",
);
