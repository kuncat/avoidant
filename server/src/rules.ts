import init, { transition } from "../rules/avoidant_competitive";
import wasm from "../rules/avoidant_competitive_bg.wasm";
let ready: Promise<unknown> | undefined;
export async function rules(state: string, command: unknown): Promise<Result> {
  ready ??= init({ module_or_path: wasm });
  await ready;
  return JSON.parse(transition(state, JSON.stringify(command)));
}
export interface Result {
  state: string;
  snapshot: Snapshot;
  error: string | null;
}
export interface Snapshot {
  revision: number;
  started: boolean;
  finished: boolean;
  deadline: number;
  startedAt: number | null;
  finishedAt: number | null;
  turnDeadline: number;
  turn: number | null;
  options: {
    mode: "collaborative" | "territory";
    bordering: boolean;
    floodLimit: number | null;
    turnBased: boolean;
    matchMinutes?: number;
    turnSeconds?: number | null;
  };
  players: {
    id: string;
    name?: string;
    score: number;
    claims: number;
    voids: number;
    sequence: number;
    cumulativeMs: number;
  }[];
  cells: { revealed: boolean; isVoid?: boolean; count?: number; owner: number | null }[];
}
