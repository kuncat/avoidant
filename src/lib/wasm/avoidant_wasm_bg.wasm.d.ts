/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export const __wbg_channel_free: (a: number, b: number) => void;
export const __wbg_channelsender_free: (a: number, b: number) => void;
export const __wbg_gamestate_free: (a: number, b: number) => void;
export const __wbg_networknode_free: (a: number, b: number) => void;
export const __wbg_pulse_free: (a: number, b: number) => void;
export const __wbg_uistate_free: (a: number, b: number) => void;
export const channel_id: (a: number) => [number, number];
export const channel_neighbors: (a: number) => [number, number];
export const channel_receiver: (a: number) => any;
export const channel_sender: (a: number) => number;
export const channel_ticket: (a: number, b: any) => [number, number, number, number];
export const channelsender_broadcast: (a: number, b: number, c: number) => any;
export const channelsender_set_nickame: (a: number, b: number, c: number) => void;
export const gamestate_applyMapCells: (a: number, b: any) => [number, number, number];
export const gamestate_cellMetadata: (a: number) => any;
export const gamestate_cells: (a: number) => any;
export const gamestate_endpointId: (a: number) => [number, number];
export const gamestate_exploreCell: (
  a: number,
  b: number,
  c: number,
  d: number,
  e: number,
) => [number, number];
export const gamestate_hasNetworkNode: (a: number) => number;
export const gamestate_invite: (a: number, b: number, c: number) => any;
export const gamestate_joinAsPeer: (a: number, b: number, c: number, d: number, e: number) => any;
export const gamestate_lastInboundMutationMs: (a: number) => [number, number];
export const gamestate_lastOutboundMutationMs: (a: number) => [number, number];
export const gamestate_networkListenerStarted: (a: number) => number;
export const gamestate_networkPeers: (a: number) => [number, number];
export const gamestate_networkSnapshot: (a: number) => any;
export const gamestate_new: (a: any) => [number, number, number];
export const gamestate_optionsFromTicket: (a: number, b: number) => [number, number, number];
export const gamestate_queueExplorePulse: (
  a: number,
  b: number,
  c: number,
  d: number,
  e: number,
) => [number, number];
export const gamestate_score: (a: number) => any;
export const gamestate_topicId: (a: number) => [number, number];
export const gamestate_uiState: (a: number) => number;
export const generateMapData: (a: any) => [number, number, number];
export const networknode_create: (a: number, b: number, c: number) => any;
export const networknode_endpointId: (a: number) => [number, number];
export const networknode_join: (a: number, b: number, c: number, d: number, e: number) => any;
export const networknode_spawn: () => any;
export const pulse_durationMs: (a: number) => number;
export const pulse_id: (a: number) => number;
export const pulse_isRemote: (a: number) => number;
export const pulse_nullPulse: () => number;
export const pulse_originCell: (a: number) => number;
export const pulse_position: (a: number) => [number, number];
export const start: () => void;
export const uistate_pulses: (a: number) => any;
export const gamestate_elevationMax: (a: number) => number;
export const gamestate_elevationMin: (a: number) => number;
export const pulse_createdAtMs: (a: number) => number;
export const pulse_maxRadius: (a: number) => number;
export const __wbg_intounderlyingbytesource_free: (a: number, b: number) => void;
export const __wbg_intounderlyingsink_free: (a: number, b: number) => void;
export const __wbg_intounderlyingsource_free: (a: number, b: number) => void;
export const intounderlyingbytesource_autoAllocateChunkSize: (a: number) => number;
export const intounderlyingbytesource_cancel: (a: number) => void;
export const intounderlyingbytesource_pull: (a: number, b: any) => any;
export const intounderlyingbytesource_start: (a: number, b: any) => void;
export const intounderlyingbytesource_type: (a: number) => number;
export const intounderlyingsink_abort: (a: number, b: any) => any;
export const intounderlyingsink_close: (a: number) => any;
export const intounderlyingsink_write: (a: number, b: any) => any;
export const intounderlyingsource_cancel: (a: number) => void;
export const intounderlyingsource_pull: (a: number, b: any) => any;
export const ring_core_0_17_14__bn_mul_mont: (
  a: number,
  b: number,
  c: number,
  d: number,
  e: number,
  f: number,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h3355929dfe4c51e3: (
  a: number,
  b: number,
  c: any,
) => [number, number];
export const wasm_bindgen__convert__closures_____invoke__h0d84f0743cb357cb: (
  a: number,
  b: number,
  c: any,
  d: any,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h4a0dba489db6b857: (
  a: number,
  b: number,
  c: any,
) => any;
export const wasm_bindgen__convert__closures_____invoke__h3bbf06cd7ec36e37: (
  a: number,
  b: number,
  c: any,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h1c58385ce92bae1d: (
  a: number,
  b: number,
  c: any,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h51eb0b0ca2400649: (
  a: number,
  b: number,
  c: any,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h5c5e22db62c3717a: (
  a: number,
  b: number,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h2c4b50ea3d5eec86: (
  a: number,
  b: number,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h25cef7a33bdbe55c: (
  a: number,
  b: number,
) => void;
export const wasm_bindgen__convert__closures_____invoke__h5b43cb68994c0bb0: (
  a: number,
  b: number,
) => void;
export const __wbindgen_malloc: (a: number, b: number) => number;
export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
export const __wbindgen_exn_store: (a: number) => void;
export const __externref_table_alloc: () => number;
export const __wbindgen_externrefs: WebAssembly.Table;
export const __externref_drop_slice: (a: number, b: number) => void;
export const __wbindgen_free: (a: number, b: number, c: number) => void;
export const __wbindgen_destroy_closure: (a: number, b: number) => void;
export const __externref_table_dealloc: (a: number) => void;
export const __wbindgen_start: () => void;
