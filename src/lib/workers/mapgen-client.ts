/**
 * Client-side helper that drives the dedicated mapgen Web Worker.
 *
 * A single worker is kept alive across calls so the wasm module is only initialized once. Each call gets a unique requestId so we can multiplex safely even though map generation is normally serial.
 */
import type { GameOptions, MapCell } from "wasm-pkg";
import type { WorkerResponse } from "$lib/workers/mapgen.worker";
import MapgenWorker from "$lib/workers/mapgen.worker?worker";

interface PendingRequest {
  resolve: (cells: MapCell[]) => void;
  reject: (error: Error) => void;
}

let workerInstance: Worker | undefined;
const pending = new Map<number, PendingRequest>();
let nextRequestId = 1;

function getWorker(): Worker {
  if (workerInstance) {
    return workerInstance;
  }

  const worker = new MapgenWorker();

  worker.addEventListener("message", (event: MessageEvent<WorkerResponse>) => {
    const data = event.data;
    const entry = pending.get(data.requestId);
    if (!entry) {
      return;
    }
    pending.delete(data.requestId);
    if (data.type === "result") {
      entry.resolve(data.cells);
    } else {
      entry.reject(new Error(data.message));
    }
  });

  const failAll = (error: Error) => {
    for (const [id, entry] of pending) {
      pending.delete(id);
      entry.reject(error);
    }
    // Drop the dead worker so the next call spawns a fresh one.
    workerInstance = undefined;
  };

  worker.addEventListener("error", (event: ErrorEvent) => {
    console.error("Mapgen worker error event", event);
    failAll(event.error instanceof Error ? event.error : new Error("Mapgen worker error"));
  });

  worker.addEventListener("messageerror", (event) => {
    console.error("Mapgen worker messageerror", event);
    failAll(new Error("Mapgen worker messageerror (structured clone failed)"));
  });

  workerInstance = worker;
  return worker;
}

export function generateMap(options: GameOptions): Promise<MapCell[]> {
  return new Promise((resolve, reject) => {
    const worker = getWorker();
    const requestId = nextRequestId++;
    pending.set(requestId, { resolve, reject });
    worker.postMessage({ type: "generate", requestId, options });
  });
}
