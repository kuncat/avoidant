/* tslint:disable */
/* eslint-disable */
/**
 * The `ReadableStreamType` enum.
 *
 * *This API requires the following crate features to be activated: `ReadableStreamType`*
 */

type ReadableStreamType = "bytes";

import type { Readable } from "svelte/store";

/**
 * Flat per-vertex terrain mesh, decoupled from cell corners.
 *
 * `positions` is a `[x, y, z]`-packed `f32` array (length is a multiple of 9 (3 verts per triangle, 3 floats per vert). `normals` is `[nx, ny, nz]`) packed and parallel to `positions`; each vertex carries a smooth normal derived analytically from the noise field so the shader can per-pixel-interpolate it and avoid faceted flat shading. `cell_indices` carries the owning cell index for each emitted vertex (length = `positions.len() / 3`). Built once per map by [`crate::mapgen::generate_terrain_triangles`].
 */
export interface TerrainTriangles {
  positions: number[];
  normals: number[];
  cellIndices: number[];
}

/**
 * Voronoi cell polygons paired with the subdivided terrain triangle mesh.
 */
export interface MapData {
  cells: MapCell[];
  terrain: TerrainTriangles;
}

export interface CellMetadataEntry {
  isExplored: boolean;
  isVoid: boolean;
  voidNeighborCount: number;
  /**
   * Toggled during a chord auto-reveal.
   */
  isRevealing: boolean;
}

export interface GameOptions {
  numCells: number;
  rngSeed: number;
  /**
   * Iroh relay server URLs to use for peer discovery and transport.
   */
  relayUrls?: string[];
  maxSamples?: number;
  slack?: number;
  /**
   * 0.0 = smooth broad hills, 1.0 = tight spiky features. Default: 0.4
   */
  spikiness?: number;
  /**
   * Minimum vertex height in world units. Default: -0.4
   */
  elevationMin?: number;
  /**
   * Maximum vertex height in world units. Default: 0.4
   */
  elevationMax?: number;
  /**
   * Per-cell terrain mesh subdivision level. Each fan triangle inside a cell is split into `S²` sub-triangles, sampling noise at every sub-vertex.
   */
  terrainSubdivisions?: number;
  voidFraction?: number;
}

export interface MapCell {
  vertices: [number, number, number][];
  neighbors: number[];
}

export interface NetworkPeerStatus {
  endpointId: string;
  nickname?: string;
  lastSeenMs?: number;
  isConnected: boolean;
}

export interface NetworkSnapshot {
  hasNode: boolean;
  listenerStarted: boolean;
  endpointId?: string;
  topicId?: string;
  peers: NetworkPeerStatus[];
  lastInboundMutationMs?: number;
  lastOutboundMutationMs?: number;
  sampledAtMs: number;
}

export interface ScoreState {
  /**
   * Risk-balanced point total. May be negative.
   *
   * - Each non-void cell explored awards [`SAFE_REWARD`] points, multiplied by a streak multiplier that grows by [`STREAK_BONUS_PER_STEP`] per consecutive non-void cell explored (capped at [`STREAK_BONUS_CAP_STEPS`] steps).
   * - Each void explore deducts a *risk-balanced* penalty equal to `reward * (1 - p) / p`, where `p` is the realized void fraction. This makes random clicking have an expected value of zero so positive scores reflect *information* the player applied, not map size.
   * - Clearing every non-void cell awards a completion bonus equal to [`COMPLETION_BONUS_FRACTION`] of the non-void-only baseline.
   */
  score: number;
  /**
   * Non-void cells explored so far.
   */
  safeExplored: number;
  /**
   * Void cells explored so far (each large-penalized and ends the streak).
   */
  voidExplored: number;
  /**
   * Current run of consecutive safe explores; resets to 0 on a void.
   */
  streak: number;
  /**
   * Longest streak achieved during this game.
   */
  bestStreak: number;
  /**
   * Total cells on the map.
   */
  totalCells: number;
  /**
   * Void cell count.
   */
  voidTotal: number;
  /**
   * True once every non-void cell has been explored.
   */
  completed: boolean;
  /**
   * `score / max_score`, in `[0, 1]`. Zero before the map has been generated or if the map contains no safe cells.
   */
  efficiency: number;
}

export class Channel {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  id(): string;
  neighbors(): string[];
  ticket(opts: any): string;
  readonly receiver: ReadableStream;
  readonly sender: ChannelSender;
}

export class ChannelSender {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  broadcast(text: string): Promise<void>;
  set_nickame(nickname: string): void;
}

export class GameState {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Apply cells produced off-thread (typically by the mapgen Web Worker).
   *
   * `cells` must be a JS array of objects matching the `MapCell` shape (i.e. `{ vertices: [[x, y, z], ...] }`).
   */
  applyMapCells(cells: MapCell[]): MapCell[];
  exploreCell(index: number, x: number, y: number, z: number): void;
  invite(nickname: string): Promise<string>;
  /**
   * Spawn a network node and join the gossip topic described by `ticket`.
   *
   * The caller is responsible for having populated the map cells (typically
   * via [`GameState::apply_map_cells`]) before invoking this; the function
   * only concerns itself with bringing the network layer online.
   */
  joinAsPeer(ticket: string, nickname: string): Promise<void>;
  constructor(options: GameOptions);
  static optionsFromTicket(ticket: string): GameOptions;
  queueExplorePulse(index: number, x: number, y: number, z: number): void;
  readonly cellMetadata: Readable<Array<CellMetadataEntry>>;
  readonly cells: Readable<Array<MapCell>>;
  readonly elevationMax: number;
  readonly elevationMin: number;
  readonly endpointId: string | undefined;
  readonly hasNetworkNode: boolean;
  readonly lastInboundMutationMs: number | undefined;
  readonly lastOutboundMutationMs: number | undefined;
  readonly networkListenerStarted: boolean;
  readonly networkPeers: NetworkPeerStatus[];
  readonly networkSnapshot: Readable<NetworkSnapshot>;
  /**
   * TODO(peer-join-sync): when peer-join state sync is implemented, the inviter should snapshot this store and send it alongside `cell_metadata` so a joining peer starts with the correct score, streak, and counters.
   */
  readonly score: Readable<ScoreState>;
  readonly topicId: string | undefined;
  readonly uiState: UiState;
}

export class IntoUnderlyingByteSource {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  cancel(): void;
  pull(controller: ReadableByteStreamController): Promise<any>;
  start(controller: ReadableByteStreamController): void;
  readonly autoAllocateChunkSize: number;
  readonly type: ReadableStreamType;
}

export class IntoUnderlyingSink {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  abort(reason: any): Promise<any>;
  close(): Promise<any>;
  write(chunk: any): Promise<any>;
}

export class IntoUnderlyingSource {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  cancel(): void;
  pull(controller: ReadableStreamDefaultController): Promise<any>;
}

export class NetworkNode {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  create(nickname: string): Promise<Channel>;
  /**
   * Returns the endpoint id of this node.
   */
  endpointId(): string;
  join(ticket: string, nickname: string): Promise<Channel>;
  /**
   * Spawns a gossip node.
   */
  static spawn(): Promise<NetworkNode>;
}

export class Pulse {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  static nullPulse(): Pulse;
  readonly createdAtMs: number;
  readonly durationMs: number;
  readonly id: number;
  readonly isRemote: boolean;
  readonly maxRadius: number;
  readonly originCell: number;
  readonly position: Float64Array;
}

export class UiState {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  readonly pulses: Readable<Array<Pulse>>;
}

/**
 * Build map cells and a subdivided terrain triangle mesh.
 */
export function generateMapData(options: GameOptions): MapData;

export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_channel_free: (a: number, b: number) => void;
  readonly __wbg_channelsender_free: (a: number, b: number) => void;
  readonly __wbg_gamestate_free: (a: number, b: number) => void;
  readonly __wbg_networknode_free: (a: number, b: number) => void;
  readonly __wbg_pulse_free: (a: number, b: number) => void;
  readonly __wbg_uistate_free: (a: number, b: number) => void;
  readonly channel_id: (a: number) => [number, number];
  readonly channel_neighbors: (a: number) => [number, number];
  readonly channel_receiver: (a: number) => any;
  readonly channel_sender: (a: number) => number;
  readonly channel_ticket: (a: number, b: any) => [number, number, number, number];
  readonly channelsender_broadcast: (a: number, b: number, c: number) => any;
  readonly channelsender_set_nickame: (a: number, b: number, c: number) => void;
  readonly gamestate_applyMapCells: (a: number, b: any) => [number, number, number];
  readonly gamestate_cellMetadata: (a: number) => any;
  readonly gamestate_cells: (a: number) => any;
  readonly gamestate_endpointId: (a: number) => [number, number];
  readonly gamestate_exploreCell: (
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
  ) => [number, number];
  readonly gamestate_hasNetworkNode: (a: number) => number;
  readonly gamestate_invite: (a: number, b: number, c: number) => any;
  readonly gamestate_joinAsPeer: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly gamestate_lastInboundMutationMs: (a: number) => [number, number];
  readonly gamestate_lastOutboundMutationMs: (a: number) => [number, number];
  readonly gamestate_networkListenerStarted: (a: number) => number;
  readonly gamestate_networkPeers: (a: number) => [number, number];
  readonly gamestate_networkSnapshot: (a: number) => any;
  readonly gamestate_new: (a: any) => [number, number, number];
  readonly gamestate_optionsFromTicket: (a: number, b: number) => [number, number, number];
  readonly gamestate_queueExplorePulse: (
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
  ) => [number, number];
  readonly gamestate_score: (a: number) => any;
  readonly gamestate_topicId: (a: number) => [number, number];
  readonly gamestate_uiState: (a: number) => number;
  readonly generateMapData: (a: any) => [number, number, number];
  readonly networknode_create: (a: number, b: number, c: number) => any;
  readonly networknode_endpointId: (a: number) => [number, number];
  readonly networknode_join: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly networknode_spawn: () => any;
  readonly pulse_durationMs: (a: number) => number;
  readonly pulse_id: (a: number) => number;
  readonly pulse_isRemote: (a: number) => number;
  readonly pulse_nullPulse: () => number;
  readonly pulse_originCell: (a: number) => number;
  readonly pulse_position: (a: number) => [number, number];
  readonly start: () => void;
  readonly uistate_pulses: (a: number) => any;
  readonly gamestate_elevationMax: (a: number) => number;
  readonly gamestate_elevationMin: (a: number) => number;
  readonly pulse_createdAtMs: (a: number) => number;
  readonly pulse_maxRadius: (a: number) => number;
  readonly __wbg_intounderlyingbytesource_free: (a: number, b: number) => void;
  readonly __wbg_intounderlyingsink_free: (a: number, b: number) => void;
  readonly __wbg_intounderlyingsource_free: (a: number, b: number) => void;
  readonly intounderlyingbytesource_autoAllocateChunkSize: (a: number) => number;
  readonly intounderlyingbytesource_cancel: (a: number) => void;
  readonly intounderlyingbytesource_pull: (a: number, b: any) => any;
  readonly intounderlyingbytesource_start: (a: number, b: any) => void;
  readonly intounderlyingbytesource_type: (a: number) => number;
  readonly intounderlyingsink_abort: (a: number, b: any) => any;
  readonly intounderlyingsink_close: (a: number) => any;
  readonly intounderlyingsink_write: (a: number, b: any) => any;
  readonly intounderlyingsource_cancel: (a: number) => void;
  readonly intounderlyingsource_pull: (a: number, b: any) => any;
  readonly ring_core_0_17_14__bn_mul_mont: (
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
    f: number,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h3355929dfe4c51e3: (
    a: number,
    b: number,
    c: any,
  ) => [number, number];
  readonly wasm_bindgen__convert__closures_____invoke__h0d84f0743cb357cb: (
    a: number,
    b: number,
    c: any,
    d: any,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h4a0dba489db6b857: (
    a: number,
    b: number,
    c: any,
  ) => any;
  readonly wasm_bindgen__convert__closures_____invoke__h3bbf06cd7ec36e37: (
    a: number,
    b: number,
    c: any,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h1c58385ce92bae1d: (
    a: number,
    b: number,
    c: any,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h51eb0b0ca2400649: (
    a: number,
    b: number,
    c: any,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h5c5e22db62c3717a: (
    a: number,
    b: number,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h2c4b50ea3d5eec86: (
    a: number,
    b: number,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h25cef7a33bdbe55c: (
    a: number,
    b: number,
  ) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h5b43cb68994c0bb0: (
    a: number,
    b: number,
  ) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __externref_drop_slice: (a: number, b: number) => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init(
  module_or_path?:
    | { module_or_path: InitInput | Promise<InitInput> }
    | InitInput
    | Promise<InitInput>,
): Promise<InitOutput>;
