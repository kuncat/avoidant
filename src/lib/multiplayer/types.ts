import type { Readable } from "svelte/store";
import type { CellMetadataEntry, MapCell, Pulse } from "$lib/wasm/avoidant_wasm";
export type BoardPulse = Pick<
  Pulse,
  "id" | "createdAtMs" | "durationMs" | "position" | "originCell" | "isRemote" | "maxRadius"
>;
export interface BoardState {
  cells: Readable<MapCell[]>;
  cellMetadata: Readable<CellMetadataEntry[]>;
  uiState: { pulses: Readable<BoardPulse[]> };
  elevationMin: number;
  elevationMax: number;
  queueExplorePulse(index: number, x: number, y: number, z: number): void;
}
export interface CompetitiveConfig {
  serverUrl: string;
  account: {
    login(register?: boolean): Promise<void>;
    logout(): Promise<void>;
    completeLogin(): Promise<void>;
    token(): Promise<string>;
    account(): Promise<{ id: string; capabilities: string[] } | null>;
  };
}
export interface MatchSnapshot {
  version: number;
  revision: number;
  started: boolean;
  finished: boolean;
  turn: number | null;
  turnDeadline: number;
  deadline: number;
  options: {
    mode: "collaborative" | "duel" | "territory";
    bordering: boolean;
    floodLimit: number | null;
    turnBased: boolean;
  };
  players: { id: string; score: number; claims: number; voids: number; sequence: number }[];
  cells: { revealed: boolean; isVoid?: boolean; count?: number; owner: number | null }[];
}
export const PLAYER_COLORS = [
  "#38bdf8",
  "#fb7185",
  "#a3e635",
  "#c084fc",
  "#fbbf24",
  "#2dd4bf",
  "#fb923c",
  "#e879f9",
];

export interface MultiplayerSetup {
  mode: "collaborative" | "duel" | "territory";
  map: string;
  bordering: boolean;
  turnBased: boolean;
  floodLimit: number | null;
}
