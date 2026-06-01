/// <reference lib="webworker" />

import init, { generateMapData, type GameOptions, type MapData } from "$lib/wasm/avoidant_wasm";

export interface GenerateMessage {
  type: "generate";
  requestId: number;
  options: GameOptions;
}

export interface ResultResponse {
  type: "result";
  requestId: number;
  cells: MapData["cells"];
  terrain: MapData["terrain"];
}

export interface ErrorResponse {
  type: "error";
  requestId: number;
  message: string;
}

export type WorkerResponse = ResultResponse | ErrorResponse;

const ctx = self as unknown as DedicatedWorkerGlobalScope;
let wasmReady: Promise<unknown> | undefined;

function ensureWasm(): Promise<unknown> {
  if (!wasmReady) {
    wasmReady = init();
  }
  return wasmReady;
}

ctx.addEventListener("message", (event: MessageEvent<GenerateMessage>) => {
  const message = event.data;
  if (!message || message.type !== "generate") {
    return;
  }

  const respond = (response: WorkerResponse) => ctx.postMessage(response);

  void (async () => {
    try {
      await ensureWasm();
      const data = generateMapData(message.options);
      const terrain: MapData["terrain"] = {
        positions: data.terrain.positions,
        normals: data.terrain.normals,
        cellIndices: data.terrain.cellIndices,
      };
      respond({
        type: "result",
        requestId: message.requestId,
        cells: data.cells,
        terrain,
      });
    } catch (error) {
      respond({
        type: "error",
        requestId: message.requestId,
        message: error instanceof Error ? `${error.name}: ${error.message}` : String(error),
      });
    }
  })();
});
