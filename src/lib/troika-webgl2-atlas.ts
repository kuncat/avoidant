/**
 * Workaround for troika-three-text failing to render in recent Brave versions.
 *
 * The `<Text>` labels (drawn via `@threlte/extras` → `troika-three-text` →
 * `webgl-sdf-generator`) stopped rendering in Brave, throwing:
 *
 *     Error: ANGLE_instanced_arrays not supported
 *
 * Root cause: `webgl-sdf-generator` renders its glyph SDF atlas into a canvas for
 * which it hard-codes a **WebGL 1** context (`getContext('webgl', …)`). On WebGL 1
 * it unconditionally requires the `ANGLE_instanced_arrays` extension — even in
 * `renderImageData`, the plain (non-instanced) blit used by its pure-JS fallback
 * path. Recent Brave builds refuse to hand out that extension (fingerprinting
 * protection returns `null` from `getExtension`), so `getExtension(...)` throws.
 * Because troika's JS fallback also routes its final atlas upload through that same
 * WebGL 1 blit, there is no working fallback and the labels never appear. Firefox
 * and Safari still expose the extension, which is why only Brave is affected.
 *
 * Fix: transparently upgrade troika's atlas canvas to a **WebGL 2** context. WebGL 2
 * has instancing built in (`vertexAttribDivisor` / `drawArraysInstanced`), so
 * `webgl-sdf-generator` never asks for `ANGLE_instanced_arrays` at all (see its
 * `isWebGL2` branches) and the GPU path works normally. Brave already exposes a
 * working WebGL 2 context here — the game's main three.js renderer uses one.
 *
 * We scope the patch as tightly as possible: it only fires for a `'webgl'` request
 * carrying troika's exact context attributes (`preserveDrawingBuffer: true` with
 * `depth: false`), which the game's own renderer (it asks for `'webgl2'` directly)
 * never matches. WebGL 2 is a superset of WebGL 1, so returning it is safe.
 *
 * Import this module for its side effect once, before any `<Text>` mounts. No-op
 * during SSR and on browsers where `'webgl'` is already the only option.
 */
export function installTroikaWebGL2AtlasFix(): void {
  if (typeof document === "undefined" || typeof HTMLCanvasElement === "undefined") {
    return; // SSR / non-DOM environment
  }
  if (typeof WebGL2RenderingContext === "undefined") {
    return; // Browser has no WebGL 2; nothing we can do, leave original behavior.
  }

  const proto = HTMLCanvasElement.prototype as HTMLCanvasElement & {
    __troikaWebGL2Patched?: boolean;
  };
  if (proto.__troikaWebGL2Patched) {
    return; // Idempotent: already installed.
  }
  proto.__troikaWebGL2Patched = true;

  const originalGetContext = proto.getContext;

  // Recognizes the atlas-canvas request that webgl-sdf-generator makes.
  function isTroikaAtlasRequest(contextType: string, attrs: unknown): boolean {
    if (contextType !== "webgl" && contextType !== "experimental-webgl") {
      return false;
    }
    const a = attrs as WebGLContextAttributes | undefined;
    return !!a && a.preserveDrawingBuffer === true && a.depth === false;
  }

  proto.getContext = function patchedGetContext(
    this: HTMLCanvasElement,
    contextType: string,
    options?: unknown,
  ) {
    if (isTroikaAtlasRequest(contextType, options)) {
      const gl2 = originalGetContext.call(this, "webgl2", options as WebGLContextAttributes);
      if (gl2) {
        return gl2;
      }
      // WebGL 2 unavailable on this canvas — fall through to the original request.
    }
    // eslint-disable-next-line prefer-rest-params
    return originalGetContext.apply(this, arguments as never);
  } as typeof proto.getContext;
}
