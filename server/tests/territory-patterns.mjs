// Render the actual board shader with the same color for every owner, so colors
// cannot disguise a repeated pattern or an owner-ID decoding regression.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import { chromium } from "playwright";
const fragment = readFileSync(
  new URL("../../src/lib/components/fragment.glsl", import.meta.url),
  "utf8",
);
const browser = await chromium.launch({
  executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  args: ["--no-sandbox"],
});
try {
  const page = await browser.newPage();
  const differences = await page.evaluate((fragment) => {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 96;
    const gl = canvas.getContext("webgl");
    if (!gl || !gl.getExtension("OES_standard_derivatives"))
      throw new Error("WebGL derivatives unavailable");
    const compile = (type, source) => {
      const shader = gl.createShader(type);
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
        throw new Error(gl.getShaderInfoLog(shader));
      return shader;
    };
    const program = gl.createProgram();
    gl.attachShader(
      program,
      compile(
        gl.VERTEX_SHADER,
        `
      attribute vec2 position;
      varying vec3 vWorldPosition;
      varying float vHeight, vEdgeDistance, vRelief, vCellIndex, vFallProgress;
      void main() {
        gl_Position = vec4(position, 0., 1.);
        vWorldPosition = vec3(0.); vHeight = 0.5; vEdgeDistance = 10.;
        vRelief = 1.; vCellIndex = 0.; vFallProgress = 0.;
      }`,
      ),
    );
    gl.attachShader(
      program,
      compile(
        gl.FRAGMENT_SHADER,
        `
      #extension GL_OES_standard_derivatives : enable
      precision highp float;
      #define MAX_PULSES 1
      #define CELL_META_VOID 0
      #define CELL_META_EXPLORED 1
      #define CELL_META_REVEALING 2
      #define SWEEP_BAND 0.12
      ${fragment}`,
      ),
    );
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS))
      throw new Error(gl.getProgramInfoLog(program));
    gl.useProgram(program);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const position = gl.getAttribLocation(program, "position");
    gl.enableVertexAttribArray(position);
    gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
    for (const [name, value] of [
      ["elevationMax", 1],
      ["uCellMetaSize", 1],
      ["uHighlightedCell", -1],
    ])
      gl.uniform1f(gl.getUniformLocation(program, name), value);
    function texture(unit, name, data) {
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl.RGBA,
        1,
        1,
        0,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        new Uint8Array(data),
      );
      gl.uniform1i(gl.getUniformLocation(program, name), unit);
    }
    texture(0, "uCellMeta", [0, 255, 0, 0]);
    const renders = [];
    for (let owner = 1; owner <= 8; owner++) {
      texture(1, "uOwnership", [128, 128, 128, owner]);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      const pixels = new Uint8Array(96 * 96 * 4);
      gl.readPixels(0, 0, 96, 96, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      renders.push(pixels);
    }
    if (gl.getError() !== gl.NO_ERROR) throw new Error("WebGL render failed");
    return renders.flatMap((a, i) =>
      renders.slice(i + 1).map((b, j) => {
        let different = 0;
        for (let p = 0; p < a.length; p += 4) if (Math.abs(a[p] - b[p]) > 5) different++;
        return { players: [i + 1, i + j + 2], fraction: different / (96 * 96) };
      }),
    );
  }, fragment);
  for (const pair of differences)
    assert.ok(
      pair.fraction > 0.15,
      `Players ${pair.players} have insufficient pattern contrast: ${pair.fraction}`,
    );
  console.log("All 28 player pairs render distinct patterns with identical territory colors.");
} finally {
  await browser.close();
}
