/* @ts-self-types="./avoidant_wasm.d.ts" */
import * as import1 from "svelte/store";
import * as import2 from "svelte/store";

export class Channel {
  static __wrap(ptr) {
    const obj = Object.create(Channel.prototype);
    obj.__wbg_ptr = ptr;
    ChannelFinalization.register(obj, obj.__wbg_ptr, obj);
    return obj;
  }
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    ChannelFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_channel_free(ptr, 0);
  }
  /**
   * @returns {string}
   */
  id() {
    let deferred1_0;
    let deferred1_1;
    try {
      const ret = wasm.channel_id(this.__wbg_ptr);
      deferred1_0 = ret[0];
      deferred1_1 = ret[1];
      return getStringFromWasm0(ret[0], ret[1]);
    } finally {
      wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
  }
  /**
   * @returns {string[]}
   */
  neighbors() {
    const ret = wasm.channel_neighbors(this.__wbg_ptr);
    var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
    return v1;
  }
  /**
   * @returns {ReadableStream}
   */
  get receiver() {
    const ret = wasm.channel_receiver(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {ChannelSender}
   */
  get sender() {
    const ret = wasm.channel_sender(this.__wbg_ptr);
    return ChannelSender.__wrap(ret);
  }
  /**
   * @param {any} opts
   * @returns {string}
   */
  ticket(opts) {
    let deferred2_0;
    let deferred2_1;
    try {
      const ret = wasm.channel_ticket(this.__wbg_ptr, opts);
      var ptr1 = ret[0];
      var len1 = ret[1];
      if (ret[3]) {
        ptr1 = 0;
        len1 = 0;
        throw takeFromExternrefTable0(ret[2]);
      }
      deferred2_0 = ptr1;
      deferred2_1 = len1;
      return getStringFromWasm0(ptr1, len1);
    } finally {
      wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
  }
}
if (Symbol.dispose) Channel.prototype[Symbol.dispose] = Channel.prototype.free;

export class ChannelSender {
  static __wrap(ptr) {
    const obj = Object.create(ChannelSender.prototype);
    obj.__wbg_ptr = ptr;
    ChannelSenderFinalization.register(obj, obj.__wbg_ptr, obj);
    return obj;
  }
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    ChannelSenderFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_channelsender_free(ptr, 0);
  }
  /**
   * @param {string} text
   * @returns {Promise<void>}
   */
  broadcast(text) {
    const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.channelsender_broadcast(this.__wbg_ptr, ptr0, len0);
    return ret;
  }
  /**
   * @param {string} nickname
   */
  set_nickame(nickname) {
    const ptr0 = passStringToWasm0(nickname, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    wasm.channelsender_set_nickame(this.__wbg_ptr, ptr0, len0);
  }
}
if (Symbol.dispose) ChannelSender.prototype[Symbol.dispose] = ChannelSender.prototype.free;

export class GameState {
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    GameStateFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_gamestate_free(ptr, 0);
  }
  /**
   * Apply cells produced off-thread (typically by the mapgen Web Worker).
   *
   * `cells` must be a JS array of objects matching the `MapCell` shape (i.e. `{ vertices: [[x, y, z], ...] }`).
   * @param {MapCell[]} cells
   * @returns {MapCell[]}
   */
  applyMapCells(cells) {
    const ret = wasm.gamestate_applyMapCells(this.__wbg_ptr, cells);
    if (ret[2]) {
      throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
  }
  /**
   * @returns {Readable<Array<CellMetadataEntry>>}
   */
  get cellMetadata() {
    const ret = wasm.gamestate_cellMetadata(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {Readable<Array<MapCell>>}
   */
  get cells() {
    const ret = wasm.gamestate_cells(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {number}
   */
  get elevationMax() {
    const ret = wasm.gamestate_elevationMax(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {number}
   */
  get elevationMin() {
    const ret = wasm.gamestate_elevationMin(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {string | undefined}
   */
  get endpointId() {
    const ret = wasm.gamestate_endpointId(this.__wbg_ptr);
    let v1;
    if (ret[0] !== 0) {
      v1 = getStringFromWasm0(ret[0], ret[1]).slice();
      wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    }
    return v1;
  }
  /**
   * @param {number} index
   * @param {number} x
   * @param {number} y
   * @param {number} z
   */
  exploreCell(index, x, y, z) {
    const ret = wasm.gamestate_exploreCell(this.__wbg_ptr, index, x, y, z);
    if (ret[1]) {
      throw takeFromExternrefTable0(ret[0]);
    }
  }
  /**
   * @returns {boolean}
   */
  get hasNetworkNode() {
    const ret = wasm.gamestate_hasNetworkNode(this.__wbg_ptr);
    return ret !== 0;
  }
  /**
   * @param {string} nickname
   * @returns {Promise<string>}
   */
  invite(nickname) {
    const ptr0 = passStringToWasm0(nickname, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.gamestate_invite(this.__wbg_ptr, ptr0, len0);
    return ret;
  }
  /**
   * Spawn a network node and join the gossip topic described by `ticket`.
   *
   * The caller is responsible for having populated the map cells (typically
   * via [`GameState::apply_map_cells`]) before invoking this; the function
   * only concerns itself with bringing the network layer online.
   * @param {string} ticket
   * @param {string} nickname
   * @returns {Promise<void>}
   */
  joinAsPeer(ticket, nickname) {
    const ptr0 = passStringToWasm0(ticket, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(nickname, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.gamestate_joinAsPeer(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    return ret;
  }
  /**
   * @returns {number | undefined}
   */
  get lastInboundMutationMs() {
    const ret = wasm.gamestate_lastInboundMutationMs(this.__wbg_ptr);
    return ret[0] === 0 ? undefined : ret[1];
  }
  /**
   * @returns {number | undefined}
   */
  get lastOutboundMutationMs() {
    const ret = wasm.gamestate_lastOutboundMutationMs(this.__wbg_ptr);
    return ret[0] === 0 ? undefined : ret[1];
  }
  /**
   * @returns {boolean}
   */
  get networkListenerStarted() {
    const ret = wasm.gamestate_networkListenerStarted(this.__wbg_ptr);
    return ret !== 0;
  }
  /**
   * @returns {NetworkPeerStatus[]}
   */
  get networkPeers() {
    const ret = wasm.gamestate_networkPeers(this.__wbg_ptr);
    var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
    return v1;
  }
  /**
   * @returns {Readable<NetworkSnapshot>}
   */
  get networkSnapshot() {
    const ret = wasm.gamestate_networkSnapshot(this.__wbg_ptr);
    return ret;
  }
  /**
   * @param {GameOptions} options
   */
  constructor(options) {
    const ret = wasm.gamestate_new(options);
    if (ret[2]) {
      throw takeFromExternrefTable0(ret[1]);
    }
    this.__wbg_ptr = ret[0];
    GameStateFinalization.register(this, this.__wbg_ptr, this);
    return this;
  }
  /**
   * @param {string} ticket
   * @returns {GameOptions}
   */
  static optionsFromTicket(ticket) {
    const ptr0 = passStringToWasm0(ticket, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.gamestate_optionsFromTicket(ptr0, len0);
    if (ret[2]) {
      throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
  }
  /**
   * @param {number} index
   * @param {number} x
   * @param {number} y
   * @param {number} z
   */
  queueExplorePulse(index, x, y, z) {
    const ret = wasm.gamestate_queueExplorePulse(this.__wbg_ptr, index, x, y, z);
    if (ret[1]) {
      throw takeFromExternrefTable0(ret[0]);
    }
  }
  /**
   * TODO(peer-join-sync): when peer-join state sync is implemented, the inviter should snapshot this store and send it alongside `cell_metadata` so a joining peer starts with the correct score, streak, and counters.
   * @returns {Readable<ScoreState>}
   */
  get score() {
    const ret = wasm.gamestate_score(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {string | undefined}
   */
  get topicId() {
    const ret = wasm.gamestate_topicId(this.__wbg_ptr);
    let v1;
    if (ret[0] !== 0) {
      v1 = getStringFromWasm0(ret[0], ret[1]).slice();
      wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    }
    return v1;
  }
  /**
   * @returns {UiState}
   */
  get uiState() {
    const ret = wasm.gamestate_uiState(this.__wbg_ptr);
    return UiState.__wrap(ret);
  }
}
if (Symbol.dispose) GameState.prototype[Symbol.dispose] = GameState.prototype.free;

export class IntoUnderlyingByteSource {
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    IntoUnderlyingByteSourceFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_intounderlyingbytesource_free(ptr, 0);
  }
  /**
   * @returns {number}
   */
  get autoAllocateChunkSize() {
    const ret = wasm.intounderlyingbytesource_autoAllocateChunkSize(this.__wbg_ptr);
    return ret >>> 0;
  }
  cancel() {
    const ptr = this.__destroy_into_raw();
    wasm.intounderlyingbytesource_cancel(ptr);
  }
  /**
   * @param {ReadableByteStreamController} controller
   * @returns {Promise<any>}
   */
  pull(controller) {
    const ret = wasm.intounderlyingbytesource_pull(this.__wbg_ptr, controller);
    return ret;
  }
  /**
   * @param {ReadableByteStreamController} controller
   */
  start(controller) {
    wasm.intounderlyingbytesource_start(this.__wbg_ptr, controller);
  }
  /**
   * @returns {ReadableStreamType}
   */
  get type() {
    const ret = wasm.intounderlyingbytesource_type(this.__wbg_ptr);
    return __wbindgen_enum_ReadableStreamType[ret];
  }
}
if (Symbol.dispose)
  IntoUnderlyingByteSource.prototype[Symbol.dispose] = IntoUnderlyingByteSource.prototype.free;

export class IntoUnderlyingSink {
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    IntoUnderlyingSinkFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_intounderlyingsink_free(ptr, 0);
  }
  /**
   * @param {any} reason
   * @returns {Promise<any>}
   */
  abort(reason) {
    const ptr = this.__destroy_into_raw();
    const ret = wasm.intounderlyingsink_abort(ptr, reason);
    return ret;
  }
  /**
   * @returns {Promise<any>}
   */
  close() {
    const ptr = this.__destroy_into_raw();
    const ret = wasm.intounderlyingsink_close(ptr);
    return ret;
  }
  /**
   * @param {any} chunk
   * @returns {Promise<any>}
   */
  write(chunk) {
    const ret = wasm.intounderlyingsink_write(this.__wbg_ptr, chunk);
    return ret;
  }
}
if (Symbol.dispose)
  IntoUnderlyingSink.prototype[Symbol.dispose] = IntoUnderlyingSink.prototype.free;

export class IntoUnderlyingSource {
  static __wrap(ptr) {
    const obj = Object.create(IntoUnderlyingSource.prototype);
    obj.__wbg_ptr = ptr;
    IntoUnderlyingSourceFinalization.register(obj, obj.__wbg_ptr, obj);
    return obj;
  }
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    IntoUnderlyingSourceFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_intounderlyingsource_free(ptr, 0);
  }
  cancel() {
    const ptr = this.__destroy_into_raw();
    wasm.intounderlyingsource_cancel(ptr);
  }
  /**
   * @param {ReadableStreamDefaultController} controller
   * @returns {Promise<any>}
   */
  pull(controller) {
    const ret = wasm.intounderlyingsource_pull(this.__wbg_ptr, controller);
    return ret;
  }
}
if (Symbol.dispose)
  IntoUnderlyingSource.prototype[Symbol.dispose] = IntoUnderlyingSource.prototype.free;

export class NetworkNode {
  static __wrap(ptr) {
    const obj = Object.create(NetworkNode.prototype);
    obj.__wbg_ptr = ptr;
    NetworkNodeFinalization.register(obj, obj.__wbg_ptr, obj);
    return obj;
  }
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    NetworkNodeFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_networknode_free(ptr, 0);
  }
  /**
   * @param {string} nickname
   * @returns {Promise<Channel>}
   */
  create(nickname) {
    const ptr0 = passStringToWasm0(nickname, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.networknode_create(this.__wbg_ptr, ptr0, len0);
    return ret;
  }
  /**
   * Returns the endpoint id of this node.
   * @returns {string}
   */
  endpointId() {
    let deferred1_0;
    let deferred1_1;
    try {
      const ret = wasm.networknode_endpointId(this.__wbg_ptr);
      deferred1_0 = ret[0];
      deferred1_1 = ret[1];
      return getStringFromWasm0(ret[0], ret[1]);
    } finally {
      wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
  }
  /**
   * @param {string} ticket
   * @param {string} nickname
   * @returns {Promise<Channel>}
   */
  join(ticket, nickname) {
    const ptr0 = passStringToWasm0(ticket, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passStringToWasm0(nickname, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.networknode_join(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    return ret;
  }
  /**
   * Spawns a gossip node.
   * @returns {Promise<NetworkNode>}
   */
  static spawn() {
    const ret = wasm.networknode_spawn();
    return ret;
  }
}
if (Symbol.dispose) NetworkNode.prototype[Symbol.dispose] = NetworkNode.prototype.free;

export class Pulse {
  static __wrap(ptr) {
    const obj = Object.create(Pulse.prototype);
    obj.__wbg_ptr = ptr;
    PulseFinalization.register(obj, obj.__wbg_ptr, obj);
    return obj;
  }
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    PulseFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_pulse_free(ptr, 0);
  }
  /**
   * @returns {number}
   */
  get createdAtMs() {
    const ret = wasm.pulse_createdAtMs(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {number}
   */
  get durationMs() {
    const ret = wasm.pulse_durationMs(this.__wbg_ptr);
    return ret >>> 0;
  }
  /**
   * @returns {number}
   */
  get id() {
    const ret = wasm.pulse_id(this.__wbg_ptr);
    return ret >>> 0;
  }
  /**
   * @returns {boolean}
   */
  get isRemote() {
    const ret = wasm.pulse_isRemote(this.__wbg_ptr);
    return ret !== 0;
  }
  /**
   * @returns {number}
   */
  get maxRadius() {
    const ret = wasm.pulse_maxRadius(this.__wbg_ptr);
    return ret;
  }
  /**
   * @returns {Pulse}
   */
  static nullPulse() {
    const ret = wasm.pulse_nullPulse();
    return Pulse.__wrap(ret);
  }
  /**
   * @returns {number}
   */
  get originCell() {
    const ret = wasm.pulse_originCell(this.__wbg_ptr);
    return ret >>> 0;
  }
  /**
   * @returns {Float64Array}
   */
  get position() {
    const ret = wasm.pulse_position(this.__wbg_ptr);
    var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
    return v1;
  }
}
if (Symbol.dispose) Pulse.prototype[Symbol.dispose] = Pulse.prototype.free;

export class UiState {
  static __wrap(ptr) {
    const obj = Object.create(UiState.prototype);
    obj.__wbg_ptr = ptr;
    UiStateFinalization.register(obj, obj.__wbg_ptr, obj);
    return obj;
  }
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    UiStateFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_uistate_free(ptr, 0);
  }
  /**
   * @returns {Readable<Array<Pulse>>}
   */
  get pulses() {
    const ret = wasm.uistate_pulses(this.__wbg_ptr);
    return ret;
  }
}
if (Symbol.dispose) UiState.prototype[Symbol.dispose] = UiState.prototype.free;

/**
 * Build map cells and a subdivided terrain triangle mesh.
 * @param {GameOptions} options
 * @returns {MapData}
 */
export function generateMapData(options) {
  const ret = wasm.generateMapData(options);
  if (ret[2]) {
    throw takeFromExternrefTable0(ret[1]);
  }
  return takeFromExternrefTable0(ret[0]);
}

export function start() {
  wasm.start();
}
function __wbg_get_imports() {
  const import0 = {
    __proto__: null,
    __wbg_Error_3639a60ed15f87e7: function (arg0, arg1) {
      const ret = Error(getStringFromWasm0(arg0, arg1));
      return ret;
    },
    __wbg_Number_a3d737fd183f7dca: function (arg0) {
      const ret = Number(arg0);
      return ret;
    },
    __wbg_String_8564e559799eccda: function (arg0, arg1) {
      const ret = String(arg1);
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg___wbindgen_bigint_get_as_i64_3af6d4ca77193a4b: function (arg0, arg1) {
      const v = arg1;
      const ret = typeof v === "bigint" ? v : undefined;
      getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
    },
    __wbg___wbindgen_boolean_get_c3dd5c39f1b5a12b: function (arg0) {
      const v = arg0;
      const ret = typeof v === "boolean" ? v : undefined;
      return isLikeNone(ret) ? 0xffffff : ret ? 1 : 0;
    },
    __wbg___wbindgen_debug_string_07cb72cfcc952e2b: function (arg0, arg1) {
      const ret = debugString(arg1);
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg___wbindgen_in_2617fa76397620d3: function (arg0, arg1) {
      const ret = arg0 in arg1;
      return ret;
    },
    __wbg___wbindgen_is_bigint_d6a8167cac401b95: function (arg0) {
      const ret = typeof arg0 === "bigint";
      return ret;
    },
    __wbg___wbindgen_is_function_2f0fd7ceb86e64c5: function (arg0) {
      const ret = typeof arg0 === "function";
      return ret;
    },
    __wbg___wbindgen_is_null_066086be3abe9bb3: function (arg0) {
      const ret = arg0 === null;
      return ret;
    },
    __wbg___wbindgen_is_object_5b22ff2418063a9c: function (arg0) {
      const val = arg0;
      const ret = typeof val === "object" && val !== null;
      return ret;
    },
    __wbg___wbindgen_is_string_eddc07a3efad52e6: function (arg0) {
      const ret = typeof arg0 === "string";
      return ret;
    },
    __wbg___wbindgen_is_undefined_244a92c34d3b6ec0: function (arg0) {
      const ret = arg0 === undefined;
      return ret;
    },
    __wbg___wbindgen_jsval_eq_403eaa3610500a25: function (arg0, arg1) {
      const ret = arg0 === arg1;
      return ret;
    },
    __wbg___wbindgen_jsval_loose_eq_1978f1e77b4bce62: function (arg0, arg1) {
      const ret = arg0 == arg1;
      return ret;
    },
    __wbg___wbindgen_number_get_dd6d69a6079f26f1: function (arg0, arg1) {
      const obj = arg1;
      const ret = typeof obj === "number" ? obj : undefined;
      getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
    },
    __wbg___wbindgen_string_get_965592073e5d848c: function (arg0, arg1) {
      const obj = arg1;
      const ret = typeof obj === "string" ? obj : undefined;
      var ptr1 = isLikeNone(ret)
        ? 0
        : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      var len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg___wbindgen_throw_9c75d47bf9e7731e: function (arg0, arg1) {
      throw new Error(getStringFromWasm0(arg0, arg1));
    },
    __wbg__wbg_cb_unref_158e43e869788cdc: function (arg0) {
      arg0._wbg_cb_unref();
    },
    __wbg_abort_43913e33ecb83d0d: function (arg0, arg1) {
      arg0.abort(arg1);
    },
    __wbg_abort_87eb7f23cf4b73d1: function (arg0) {
      arg0.abort();
    },
    __wbg_addEventListener_8b7ec0528991cf78: function () {
      return handleError(function (arg0, arg1, arg2, arg3) {
        arg0.addEventListener(getStringFromWasm0(arg1, arg2), arg3);
      }, arguments);
    },
    __wbg_append_8df396311184f750: function () {
      return handleError(function (arg0, arg1, arg2, arg3, arg4) {
        arg0.append(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
      }, arguments);
    },
    __wbg_arrayBuffer_87e3ac06d961f7a0: function () {
      return handleError(function (arg0) {
        const ret = arg0.arrayBuffer();
        return ret;
      }, arguments);
    },
    __wbg_body_6929614c20dfa7b0: function (arg0) {
      const ret = arg0.body;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    },
    __wbg_buffer_9ee17426fe5a5d65: function (arg0) {
      const ret = arg0.buffer;
      return ret;
    },
    __wbg_byobRequest_178b64c09a0bee03: function (arg0) {
      const ret = arg0.byobRequest;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    },
    __wbg_byteLength_1f57c71e64ee0180: function (arg0) {
      const ret = arg0.byteLength;
      return ret;
    },
    __wbg_byteOffset_648d0af273024f3d: function (arg0) {
      const ret = arg0.byteOffset;
      return ret;
    },
    __wbg_call_a41d6421b30a32c5: function () {
      return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.call(arg1, arg2);
        return ret;
      }, arguments);
    },
    __wbg_call_add9e5a76382e668: function () {
      return handleError(function (arg0, arg1) {
        const ret = arg0.call(arg1);
        return ret;
      }, arguments);
    },
    __wbg_cancel_f97a3ee5a8b30eef: function (arg0) {
      const ret = arg0.cancel();
      return ret;
    },
    __wbg_catch_f939343cb181958c: function (arg0, arg1) {
      const ret = arg0.catch(arg1);
      return ret;
    },
    __wbg_channel_new: function (arg0) {
      const ret = Channel.__wrap(arg0);
      return ret;
    },
    __wbg_clearTimeout_1ccca1faf41fc6f8: function (arg0) {
      const ret = clearTimeout(arg0);
      return ret;
    },
    __wbg_clearTimeout_47a40e3be01ed7a3: function () {
      return handleError(function (arg0, arg1) {
        arg0.clearTimeout(arg1);
      }, arguments);
    },
    __wbg_close_63e009c5a75f5597: function () {
      return handleError(function (arg0) {
        arg0.close();
      }, arguments);
    },
    __wbg_close_931d0c62e2aab92c: function () {
      return handleError(function (arg0) {
        arg0.close();
      }, arguments);
    },
    __wbg_close_de471367367aa5cb: function () {
      return handleError(function (arg0) {
        arg0.close();
      }, arguments);
    },
    __wbg_code_be6f339819ebb2c4: function (arg0) {
      const ret = arg0.code;
      return ret;
    },
    __wbg_code_f1d2ddc1fbbb5aad: function (arg0) {
      const ret = arg0.code;
      return ret;
    },
    __wbg_crypto_38df2bab126b63dc: function (arg0) {
      const ret = arg0.crypto;
      return ret;
    },
    __wbg_data_4a14fad4c5f216c4: function (arg0) {
      const ret = arg0.data;
      return ret;
    },
    __wbg_debug_eaef3b49d572d680: function (arg0, arg1) {
      var v0 = getArrayJsValueFromWasm0(arg0, arg1).slice();
      wasm.__wbindgen_free(arg0, arg1 * 4, 4);
      console.debug(...v0);
    },
    __wbg_done_b1afd6201ac045e0: function (arg0) {
      const ret = arg0.done;
      return ret;
    },
    __wbg_enqueue_6c7cd543c0f3828e: function () {
      return handleError(function (arg0, arg1) {
        arg0.enqueue(arg1);
      }, arguments);
    },
    __wbg_entries_83f42485034accab: function (arg0) {
      const ret = arg0.entries();
      return ret;
    },
    __wbg_entries_bb9843ba73dc70d6: function (arg0) {
      const ret = Object.entries(arg0);
      return ret;
    },
    __wbg_error_71b0e71161a5f3a0: function (arg0, arg1) {
      var v0 = getArrayJsValueFromWasm0(arg0, arg1).slice();
      wasm.__wbindgen_free(arg0, arg1 * 4, 4);
      console.error(...v0);
    },
    __wbg_error_a6fa202b58aa1cd3: function (arg0, arg1) {
      let deferred0_0;
      let deferred0_1;
      try {
        deferred0_0 = arg0;
        deferred0_1 = arg1;
        console.error(getStringFromWasm0(arg0, arg1));
      } finally {
        wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
      }
    },
    __wbg_fetch_1a030943aa8e0c38: function (arg0, arg1) {
      const ret = arg0.fetch(arg1);
      return ret;
    },
    __wbg_fetch_c6486a0142348bc8: function (arg0) {
      const ret = fetch(arg0);
      return ret;
    },
    __wbg_getRandomValues_76dfc69825c9c552: function () {
      return handleError(function (arg0, arg1) {
        globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
      }, arguments);
    },
    __wbg_getRandomValues_c44a50d8cfdaebeb: function () {
      return handleError(function (arg0, arg1) {
        arg0.getRandomValues(arg1);
      }, arguments);
    },
    __wbg_getReader_9facd4f899beac89: function () {
      return handleError(function (arg0) {
        const ret = arg0.getReader();
        return ret;
      }, arguments);
    },
    __wbg_get_41476db20fef99a8: function () {
      return handleError(function (arg0, arg1) {
        const ret = Reflect.get(arg0, arg1);
        return ret;
      }, arguments);
    },
    __wbg_get_652f640b3b0b6e3e: function (arg0, arg1) {
      const ret = arg0[arg1 >>> 0];
      return ret;
    },
    __wbg_get_9cfea9b7bbf12a15: function () {
      return handleError(function (arg0, arg1) {
        const ret = Reflect.get(arg0, arg1);
        return ret;
      }, arguments);
    },
    __wbg_get_done_2088079830fb242e: function (arg0) {
      const ret = arg0.done;
      return isLikeNone(ret) ? 0xffffff : ret ? 1 : 0;
    },
    __wbg_get_unchecked_be562b1421656321: function (arg0, arg1) {
      const ret = arg0[arg1 >>> 0];
      return ret;
    },
    __wbg_get_value_52f4b39f58a812ed: function (arg0) {
      const ret = arg0.value;
      return ret;
    },
    __wbg_get_with_ref_key_6412cf3094599694: function (arg0, arg1) {
      const ret = arg0[arg1];
      return ret;
    },
    __wbg_has_3a6f31f647e0ba22: function () {
      return handleError(function (arg0, arg1) {
        const ret = Reflect.has(arg0, arg1);
        return ret;
      }, arguments);
    },
    __wbg_headers_de17f740bce997ae: function (arg0) {
      const ret = arg0.headers;
      return ret;
    },
    __wbg_instanceof_ArrayBuffer_eab9f28fbec23477: function (arg0) {
      let result;
      try {
        result = arg0 instanceof ArrayBuffer;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    },
    __wbg_instanceof_Blob_03470b25075ee8f1: function (arg0) {
      let result;
      try {
        result = arg0 instanceof Blob;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    },
    __wbg_instanceof_Map_10d4edf60fcf9327: function (arg0) {
      let result;
      try {
        result = arg0 instanceof Map;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    },
    __wbg_instanceof_Response_370b83aa6c17e88a: function (arg0) {
      let result;
      try {
        result = arg0 instanceof Response;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    },
    __wbg_instanceof_Uint8Array_57d77acd50e4c44d: function (arg0) {
      let result;
      try {
        result = arg0 instanceof Uint8Array;
      } catch (_) {
        result = false;
      }
      const ret = result;
      return ret;
    },
    __wbg_isArray_c6c6ef8308995bcf: function (arg0) {
      const ret = Array.isArray(arg0);
      return ret;
    },
    __wbg_isSafeInteger_3c56c421a5b4cce4: function (arg0) {
      const ret = Number.isSafeInteger(arg0);
      return ret;
    },
    __wbg_iterator_9d68985a1d096fc2: function () {
      const ret = Symbol.iterator;
      return ret;
    },
    __wbg_length_0a6ce016dc1460b0: function (arg0) {
      const ret = arg0.length;
      return ret;
    },
    __wbg_length_ba3c032602efe310: function (arg0) {
      const ret = arg0.length;
      return ret;
    },
    __wbg_log_7a0760e115750083: function (arg0, arg1) {
      var v0 = getArrayJsValueFromWasm0(arg0, arg1).slice();
      wasm.__wbindgen_free(arg0, arg1 * 4, 4);
      console.log(...v0);
    },
    __wbg_message_609b498da776cb30: function (arg0, arg1) {
      const ret = arg1.message;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg_msCrypto_bd5a034af96bcba6: function (arg0) {
      const ret = arg0.msCrypto;
      return ret;
    },
    __wbg_networknode_new: function (arg0) {
      const ret = NetworkNode.__wrap(arg0);
      return ret;
    },
    __wbg_new_18865c63fa645c6f: function () {
      return handleError(function () {
        const ret = new Headers();
        return ret;
      }, arguments);
    },
    __wbg_new_227d7c05414eb861: function () {
      const ret = new Error();
      return ret;
    },
    __wbg_new_2fad8ca02fd00684: function () {
      const ret = new Object();
      return ret;
    },
    __wbg_new_3baa8d9866155c79: function () {
      const ret = new Array();
      return ret;
    },
    __wbg_new_51ff470dc2f61e27: function () {
      return handleError(function () {
        const ret = new AbortController();
        return ret;
      }, arguments);
    },
    __wbg_new_71b820e9c1f9ee88: function () {
      return handleError(function (arg0, arg1) {
        const ret = new WebSocket(getStringFromWasm0(arg0, arg1));
        return ret;
      }, arguments);
    },
    __wbg_new_8454eee672b2ba6e: function (arg0) {
      const ret = new Uint8Array(arg0);
      return ret;
    },
    __wbg_new_c9ea13ea803a692e: function (arg0, arg1) {
      const ret = new Error(getStringFromWasm0(arg0, arg1));
      return ret;
    },
    __wbg_new_from_slice_5a173c243af2e823: function (arg0, arg1) {
      const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
      return ret;
    },
    __wbg_new_typed_1137602701dc87d4: function (arg0, arg1) {
      try {
        var state0 = { a: arg0, b: arg1 };
        var cb0 = (arg0, arg1) => {
          const a = state0.a;
          state0.a = 0;
          try {
            return wasm_bindgen__convert__closures_____invoke__h0d84f0743cb357cb(
              a,
              state0.b,
              arg0,
              arg1,
            );
          } finally {
            state0.a = a;
          }
        };
        const ret = new Promise(cb0);
        return ret;
      } finally {
        state0.a = 0;
      }
    },
    __wbg_new_with_byte_offset_and_length_643e5e9e2fb6b1ad: function (arg0, arg1, arg2) {
      const ret = new Uint8Array(arg0, arg1 >>> 0, arg2 >>> 0);
      return ret;
    },
    __wbg_new_with_into_underlying_source_fd904252f385f59c: function (arg0, arg1) {
      const ret = new ReadableStream(IntoUnderlyingSource.__wrap(arg0), arg1);
      return ret;
    },
    __wbg_new_with_length_9011f5da794bf5d9: function (arg0) {
      const ret = new Uint8Array(arg0 >>> 0);
      return ret;
    },
    __wbg_new_with_str_and_init_da311e12114f4d1e: function () {
      return handleError(function (arg0, arg1, arg2) {
        const ret = new Request(getStringFromWasm0(arg0, arg1), arg2);
        return ret;
      }, arguments);
    },
    __wbg_new_with_str_sequence_d90cb07368a00c61: function () {
      return handleError(function (arg0, arg1, arg2) {
        const ret = new WebSocket(getStringFromWasm0(arg0, arg1), arg2);
        return ret;
      }, arguments);
    },
    __wbg_next_261c3c48c6e309a5: function (arg0) {
      const ret = arg0.next;
      return ret;
    },
    __wbg_next_aacee310bcfe6461: function () {
      return handleError(function (arg0) {
        const ret = arg0.next();
        return ret;
      }, arguments);
    },
    __wbg_node_84ea875411254db1: function (arg0) {
      const ret = arg0.node;
      return ret;
    },
    __wbg_now_4f457f10f864aec5: function () {
      const ret = Date.now();
      return ret;
    },
    __wbg_now_e7c6795a7f81e10f: function (arg0) {
      const ret = arg0.now();
      return ret;
    },
    __wbg_parse_342d5616e14beccc: function () {
      return handleError(function (arg0, arg1) {
        const ret = JSON.parse(getStringFromWasm0(arg0, arg1));
        return ret;
      }, arguments);
    },
    __wbg_performance_3fcf6e32a7e1ed0a: function (arg0) {
      const ret = arg0.performance;
      return ret;
    },
    __wbg_process_44c7a14e11e9f69e: function (arg0) {
      const ret = arg0.process;
      return ret;
    },
    __wbg_protocol_db28bce84eb5b12a: function (arg0, arg1) {
      const ret = arg1.protocol;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg_prototypesetcall_fd4050e806e1d519: function (arg0, arg1, arg2) {
      Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
    },
    __wbg_pulse_new: function (arg0) {
      const ret = Pulse.__wrap(arg0);
      return ret;
    },
    __wbg_push_60a5366c0bb22a7d: function (arg0, arg1) {
      const ret = arg0.push(arg1);
      return ret;
    },
    __wbg_queueMicrotask_40ac6ffc2848ba77: function (arg0) {
      queueMicrotask(arg0);
    },
    __wbg_queueMicrotask_74d092439f6494c1: function (arg0) {
      const ret = arg0.queueMicrotask;
      return ret;
    },
    __wbg_randomFillSync_6c25eac9869eb53c: function () {
      return handleError(function (arg0, arg1) {
        arg0.randomFillSync(arg1);
      }, arguments);
    },
    __wbg_read_ac2e4325f1799cbe: function (arg0) {
      const ret = arg0.read();
      return ret;
    },
    __wbg_readyState_be3cc9403da6c6ae: function (arg0) {
      const ret = arg0.readyState;
      return ret;
    },
    __wbg_reason_fe958bcb63725f3b: function (arg0, arg1) {
      const ret = arg1.reason;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg_releaseLock_9e0ebc0b5270a358: function (arg0) {
      arg0.releaseLock();
    },
    __wbg_removeEventListener_83bbb9ee36238073: function () {
      return handleError(function (arg0, arg1, arg2, arg3) {
        arg0.removeEventListener(getStringFromWasm0(arg1, arg2), arg3);
      }, arguments);
    },
    __wbg_require_b4edbdcf3e2a1ef0: function () {
      return handleError(function () {
        const ret = module.require;
        return ret;
      }, arguments);
    },
    __wbg_resolve_9feb5d906ca62419: function (arg0) {
      const ret = Promise.resolve(arg0);
      return ret;
    },
    __wbg_respond_e7e53102735b2ae2: function () {
      return handleError(function (arg0, arg1) {
        arg0.respond(arg1 >>> 0);
      }, arguments);
    },
    __wbg_send_0edb796d05cd3239: function () {
      return handleError(function (arg0, arg1, arg2) {
        arg0.send(getStringFromWasm0(arg1, arg2));
      }, arguments);
    },
    __wbg_send_c422d0aa0cb71d09: function () {
      return handleError(function (arg0, arg1, arg2) {
        arg0.send(getArrayU8FromWasm0(arg1, arg2));
      }, arguments);
    },
    __wbg_setTimeout_30be5552e4410378: function (arg0, arg1) {
      const ret = setTimeout(arg0, arg1);
      return ret;
    },
    __wbg_setTimeout_6613a51400c1bf9f: function () {
      return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.setTimeout(arg1, arg2);
        return ret;
      }, arguments);
    },
    __wbg_set_6be42768c690e380: function (arg0, arg1, arg2) {
      arg0[arg1] = arg2;
    },
    __wbg_set_b0d9dc239ecdb765: function (arg0, arg1, arg2) {
      arg0.set(getArrayU8FromWasm0(arg1, arg2));
    },
    __wbg_set_binaryType_8564bdba0fbec720: function (arg0, arg1) {
      arg0.binaryType = __wbindgen_enum_BinaryType[arg1];
    },
    __wbg_set_body_aaff4f5f9991f342: function (arg0, arg1) {
      arg0.body = arg1;
    },
    __wbg_set_cache_d1f2b7b4dfa39317: function (arg0, arg1) {
      arg0.cache = __wbindgen_enum_RequestCache[arg1];
    },
    __wbg_set_credentials_f31e4d30b974ce14: function (arg0, arg1) {
      arg0.credentials = __wbindgen_enum_RequestCredentials[arg1];
    },
    __wbg_set_eca238efa99478e2: function (arg0, arg1) {
      arg0.set(arg1);
    },
    __wbg_set_f614f6a0608d1d1d: function (arg0, arg1, arg2) {
      arg0[arg1 >>> 0] = arg2;
    },
    __wbg_set_handle_event_ce9002ca160b1275: function (arg0, arg1) {
      arg0.handleEvent = arg1;
    },
    __wbg_set_headers_ae96049ea40e9eef: function (arg0, arg1) {
      arg0.headers = arg1;
    },
    __wbg_set_high_water_mark_84684938153a659a: function (arg0, arg1) {
      arg0.highWaterMark = arg1;
    },
    __wbg_set_method_0eea8a5597775fa1: function (arg0, arg1, arg2) {
      arg0.method = getStringFromWasm0(arg1, arg2);
    },
    __wbg_set_mode_9fe47bff60a1580d: function (arg0, arg1) {
      arg0.mode = __wbindgen_enum_RequestMode[arg1];
    },
    __wbg_set_onclose_f756840519cd20b5: function (arg0, arg1) {
      arg0.onclose = arg1;
    },
    __wbg_set_onerror_02f33de339f1fa31: function (arg0, arg1) {
      arg0.onerror = arg1;
    },
    __wbg_set_onmessage_d2ff0c1d20584625: function (arg0, arg1) {
      arg0.onmessage = arg1;
    },
    __wbg_set_onopen_1da8a4f65e6180d2: function (arg0, arg1) {
      arg0.onopen = arg1;
    },
    __wbg_set_signal_8c5cf4c3b27bd8a8: function (arg0, arg1) {
      arg0.signal = arg1;
    },
    __wbg_signal_4643ce883b92b553: function (arg0) {
      const ret = arg0.signal;
      return ret;
    },
    __wbg_stack_3b0d974bbf31e44f: function (arg0, arg1) {
      const ret = arg1.stack;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg_static_accessor_GLOBAL_THIS_1c7f1bd6c6941fdb: function () {
      const ret = typeof globalThis === "undefined" ? null : globalThis;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    },
    __wbg_static_accessor_GLOBAL_e039bc914f83e74e: function () {
      const ret = typeof global === "undefined" ? null : global;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    },
    __wbg_static_accessor_SELF_8bf8c48c28420ad5: function () {
      const ret = typeof self === "undefined" ? null : self;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    },
    __wbg_static_accessor_WINDOW_6aeee9b51652ee0f: function () {
      const ret = typeof window === "undefined" ? null : window;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    },
    __wbg_status_157e67ab07d01f8a: function (arg0) {
      const ret = arg0.status;
      return ret;
    },
    __wbg_stringify_7fd5cae8859a6f10: function () {
      return handleError(function (arg0) {
        const ret = JSON.stringify(arg0);
        return ret;
      }, arguments);
    },
    __wbg_subarray_fbe3cef290e1fa43: function (arg0, arg1, arg2) {
      const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
      return ret;
    },
    __wbg_then_20a157d939b514f5: function (arg0, arg1) {
      const ret = arg0.then(arg1);
      return ret;
    },
    __wbg_then_5ef9b762bc91555c: function (arg0, arg1, arg2) {
      const ret = arg0.then(arg1, arg2);
      return ret;
    },
    __wbg_url_68fd9a221360e0db: function (arg0, arg1) {
      const ret = arg1.url;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg_url_a0e994e7d0317efc: function (arg0, arg1) {
      const ret = arg1.url;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg_value_f852716acdeb3e82: function (arg0) {
      const ret = arg0.value;
      return ret;
    },
    __wbg_versions_276b2795b1c6a219: function (arg0) {
      const ret = arg0.versions;
      return ret;
    },
    __wbg_view_16bd97d49793e1a9: function (arg0) {
      const ret = arg0.view;
      return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
    },
    __wbg_warn_3a37cdd7216f1479: function (arg0, arg1) {
      var v0 = getArrayJsValueFromWasm0(arg0, arg1).slice();
      wasm.__wbindgen_free(arg0, arg1 * 4, 4);
      console.warn(...v0);
    },
    __wbg_wasClean_92b4133f985dfae0: function (arg0) {
      const ret = arg0.wasClean;
      return ret;
    },
    __wbindgen_cast_0000000000000001: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 2264, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h3bbf06cd7ec36e37,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000002: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 3863, ret: Externref, inner_ret: Some(Externref) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h4a0dba489db6b857,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000003: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 3864, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h3355929dfe4c51e3,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000004: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("CloseEvent")], shim_idx: 1061, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h1c58385ce92bae1d,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000005: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("MessageEvent")], shim_idx: 2657, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h51eb0b0ca2400649,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000006: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 2223, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h5c5e22db62c3717a,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000007: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 2405, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h2c4b50ea3d5eec86,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000008: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 2407, ret: Unit, inner_ret: Some(Unit) }, mutable: false }) -> Externref`.
      const ret = makeClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h25cef7a33bdbe55c,
      );
      return ret;
    },
    __wbindgen_cast_0000000000000009: function (arg0, arg1) {
      // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 3737, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
      const ret = makeMutClosure(
        arg0,
        arg1,
        wasm_bindgen__convert__closures_____invoke__h5b43cb68994c0bb0,
      );
      return ret;
    },
    __wbindgen_cast_000000000000000a: function (arg0) {
      // Cast intrinsic for `F64 -> Externref`.
      const ret = arg0;
      return ret;
    },
    __wbindgen_cast_000000000000000b: function (arg0) {
      // Cast intrinsic for `I64 -> Externref`.
      const ret = arg0;
      return ret;
    },
    __wbindgen_cast_000000000000000c: function (arg0, arg1) {
      // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
      const ret = getArrayU8FromWasm0(arg0, arg1);
      return ret;
    },
    __wbindgen_cast_000000000000000d: function (arg0, arg1) {
      // Cast intrinsic for `Ref(String) -> Externref`.
      const ret = getStringFromWasm0(arg0, arg1);
      return ret;
    },
    __wbindgen_cast_000000000000000e: function (arg0) {
      // Cast intrinsic for `U64 -> Externref`.
      const ret = BigInt.asUintN(64, arg0);
      return ret;
    },
    __wbindgen_init_externref_table: function () {
      const table = wasm.__wbindgen_externrefs;
      const offset = table.grow(4);
      table.set(0, undefined);
      table.set(offset + 0, undefined);
      table.set(offset + 1, null);
      table.set(offset + 2, true);
      table.set(offset + 3, false);
    },
  };
  return {
    __proto__: null,
    "./avoidant_wasm_bg.js": import0,
    "svelte/store": import1,
    "svelte/store": import2,
  };
}

function wasm_bindgen__convert__closures_____invoke__h5c5e22db62c3717a(arg0, arg1) {
  wasm.wasm_bindgen__convert__closures_____invoke__h5c5e22db62c3717a(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__h2c4b50ea3d5eec86(arg0, arg1) {
  wasm.wasm_bindgen__convert__closures_____invoke__h2c4b50ea3d5eec86(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__h25cef7a33bdbe55c(arg0, arg1) {
  wasm.wasm_bindgen__convert__closures_____invoke__h25cef7a33bdbe55c(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__h5b43cb68994c0bb0(arg0, arg1) {
  wasm.wasm_bindgen__convert__closures_____invoke__h5b43cb68994c0bb0(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__h3bbf06cd7ec36e37(arg0, arg1, arg2) {
  wasm.wasm_bindgen__convert__closures_____invoke__h3bbf06cd7ec36e37(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h1c58385ce92bae1d(arg0, arg1, arg2) {
  wasm.wasm_bindgen__convert__closures_____invoke__h1c58385ce92bae1d(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h51eb0b0ca2400649(arg0, arg1, arg2) {
  wasm.wasm_bindgen__convert__closures_____invoke__h51eb0b0ca2400649(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h4a0dba489db6b857(arg0, arg1, arg2) {
  const ret = wasm.wasm_bindgen__convert__closures_____invoke__h4a0dba489db6b857(arg0, arg1, arg2);
  return ret;
}

function wasm_bindgen__convert__closures_____invoke__h3355929dfe4c51e3(arg0, arg1, arg2) {
  const ret = wasm.wasm_bindgen__convert__closures_____invoke__h3355929dfe4c51e3(arg0, arg1, arg2);
  if (ret[1]) {
    throw takeFromExternrefTable0(ret[0]);
  }
}

function wasm_bindgen__convert__closures_____invoke__h0d84f0743cb357cb(arg0, arg1, arg2, arg3) {
  wasm.wasm_bindgen__convert__closures_____invoke__h0d84f0743cb357cb(arg0, arg1, arg2, arg3);
}

const __wbindgen_enum_BinaryType = ["blob", "arraybuffer"];

const __wbindgen_enum_ReadableStreamType = ["bytes"];

const __wbindgen_enum_RequestCache = [
  "default",
  "no-store",
  "reload",
  "no-cache",
  "force-cache",
  "only-if-cached",
];

const __wbindgen_enum_RequestCredentials = ["omit", "same-origin", "include"];

const __wbindgen_enum_RequestMode = ["same-origin", "no-cors", "cors", "navigate"];
const ChannelFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_channel_free(ptr, 1));
const ChannelSenderFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_channelsender_free(ptr, 1));
const GameStateFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_gamestate_free(ptr, 1));
const IntoUnderlyingByteSourceFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_intounderlyingbytesource_free(ptr, 1));
const IntoUnderlyingSinkFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_intounderlyingsink_free(ptr, 1));
const IntoUnderlyingSourceFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_intounderlyingsource_free(ptr, 1));
const NetworkNodeFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_networknode_free(ptr, 1));
const PulseFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_pulse_free(ptr, 1));
const UiStateFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((ptr) => wasm.__wbg_uistate_free(ptr, 1));

function addToExternrefTable0(obj) {
  const idx = wasm.__externref_table_alloc();
  wasm.__wbindgen_externrefs.set(idx, obj);
  return idx;
}

const CLOSURE_DTORS =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((state) => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
  // primitive types
  const type = typeof val;
  if (type == "number" || type == "boolean" || val == null) {
    return `${val}`;
  }
  if (type == "string") {
    return `"${val}"`;
  }
  if (type == "symbol") {
    const description = val.description;
    if (description == null) {
      return "Symbol";
    } else {
      return `Symbol(${description})`;
    }
  }
  if (type == "function") {
    const name = val.name;
    if (typeof name == "string" && name.length > 0) {
      return `Function(${name})`;
    } else {
      return "Function";
    }
  }
  // objects
  if (Array.isArray(val)) {
    const length = val.length;
    let debug = "[";
    if (length > 0) {
      debug += debugString(val[0]);
    }
    for (let i = 1; i < length; i++) {
      debug += ", " + debugString(val[i]);
    }
    debug += "]";
    return debug;
  }
  // Test for built-in
  const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
  let className;
  if (builtInMatches && builtInMatches.length > 1) {
    className = builtInMatches[1];
  } else {
    // Failed to match the standard '[object ClassName]'
    return toString.call(val);
  }
  if (className == "Object") {
    // we're a user defined class or Object
    // JSON.stringify avoids problems with cycles, and is generally much
    // easier than looping through ownProperties of `val`.
    try {
      return "Object(" + JSON.stringify(val) + ")";
    } catch (_) {
      return "Object";
    }
  }
  // errors
  if (val instanceof Error) {
    return `${val.name}: ${val.message}\n${val.stack}`;
  }
  // TODO we could test for more things here, like `Set`s and `Map`s.
  return className;
}

function getArrayF64FromWasm0(ptr, len) {
  ptr = ptr >>> 0;
  return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

function getArrayJsValueFromWasm0(ptr, len) {
  ptr = ptr >>> 0;
  const mem = getDataViewMemory0();
  const result = [];
  for (let i = ptr; i < ptr + 4 * len; i += 4) {
    result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
  }
  wasm.__externref_drop_slice(ptr, len);
  return result;
}

function getArrayU8FromWasm0(ptr, len) {
  ptr = ptr >>> 0;
  return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
  if (
    cachedDataViewMemory0 === null ||
    cachedDataViewMemory0.buffer.detached === true ||
    (cachedDataViewMemory0.buffer.detached === undefined &&
      cachedDataViewMemory0.buffer !== wasm.memory.buffer)
  ) {
    cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
  }
  return cachedDataViewMemory0;
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
  if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
    cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
  }
  return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
  return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
  if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
    cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
  }
  return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
  try {
    return f.apply(this, args);
  } catch (e) {
    const idx = addToExternrefTable0(e);
    wasm.__wbindgen_exn_store(idx);
  }
}

function isLikeNone(x) {
  return x === undefined || x === null;
}

function makeClosure(arg0, arg1, f) {
  const state = { a: arg0, b: arg1, cnt: 1 };
  const real = (...args) => {
    // First up with a closure we increment the internal reference
    // count. This ensures that the Rust closure environment won't
    // be deallocated while we're invoking it.
    state.cnt++;
    try {
      return f(state.a, state.b, ...args);
    } finally {
      real._wbg_cb_unref();
    }
  };
  real._wbg_cb_unref = () => {
    if (--state.cnt === 0) {
      wasm.__wbindgen_destroy_closure(state.a, state.b);
      state.a = 0;
      CLOSURE_DTORS.unregister(state);
    }
  };
  CLOSURE_DTORS.register(real, state, state);
  return real;
}

function makeMutClosure(arg0, arg1, f) {
  const state = { a: arg0, b: arg1, cnt: 1 };
  const real = (...args) => {
    // First up with a closure we increment the internal reference
    // count. This ensures that the Rust closure environment won't
    // be deallocated while we're invoking it.
    state.cnt++;
    const a = state.a;
    state.a = 0;
    try {
      return f(a, state.b, ...args);
    } finally {
      state.a = a;
      real._wbg_cb_unref();
    }
  };
  real._wbg_cb_unref = () => {
    if (--state.cnt === 0) {
      wasm.__wbindgen_destroy_closure(state.a, state.b);
      state.a = 0;
      CLOSURE_DTORS.unregister(state);
    }
  };
  CLOSURE_DTORS.register(real, state, state);
  return real;
}

function passStringToWasm0(arg, malloc, realloc) {
  if (realloc === undefined) {
    const buf = cachedTextEncoder.encode(arg);
    const ptr = malloc(buf.length, 1) >>> 0;
    getUint8ArrayMemory0()
      .subarray(ptr, ptr + buf.length)
      .set(buf);
    WASM_VECTOR_LEN = buf.length;
    return ptr;
  }

  let len = arg.length;
  let ptr = malloc(len, 1) >>> 0;

  const mem = getUint8ArrayMemory0();

  let offset = 0;

  for (; offset < len; offset++) {
    const code = arg.charCodeAt(offset);
    if (code > 0x7f) break;
    mem[ptr + offset] = code;
  }
  if (offset !== len) {
    if (offset !== 0) {
      arg = arg.slice(offset);
    }
    ptr = realloc(ptr, len, (len = offset + arg.length * 3), 1) >>> 0;
    const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
    const ret = cachedTextEncoder.encodeInto(arg, view);

    offset += ret.written;
    ptr = realloc(ptr, len, offset, 1) >>> 0;
  }

  WASM_VECTOR_LEN = offset;
  return ptr;
}

function takeFromExternrefTable0(idx) {
  const value = wasm.__wbindgen_externrefs.get(idx);
  wasm.__externref_table_dealloc(idx);
  return value;
}

let cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
  numBytesDecoded += len;
  if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
    cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
    cachedTextDecoder.decode();
    numBytesDecoded = len;
  }
  return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!("encodeInto" in cachedTextEncoder)) {
  cachedTextEncoder.encodeInto = function (arg, view) {
    const buf = cachedTextEncoder.encode(arg);
    view.set(buf);
    return {
      read: arg.length,
      written: buf.length,
    };
  };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
  wasmInstance = instance;
  wasm = instance.exports;
  wasmModule = module;
  cachedDataViewMemory0 = null;
  cachedFloat64ArrayMemory0 = null;
  cachedUint8ArrayMemory0 = null;
  wasm.__wbindgen_start();
  return wasm;
}

async function __wbg_load(module, imports) {
  if (typeof Response === "function" && module instanceof Response) {
    if (typeof WebAssembly.instantiateStreaming === "function") {
      try {
        return await WebAssembly.instantiateStreaming(module, imports);
      } catch (e) {
        const validResponse = module.ok && expectedResponseType(module.type);

        if (validResponse && module.headers.get("Content-Type") !== "application/wasm") {
          console.warn(
            "`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n",
            e,
          );
        } else {
          throw e;
        }
      }
    }

    const bytes = await module.arrayBuffer();
    return await WebAssembly.instantiate(bytes, imports);
  } else {
    const instance = await WebAssembly.instantiate(module, imports);

    if (instance instanceof WebAssembly.Instance) {
      return { instance, module };
    } else {
      return instance;
    }
  }

  function expectedResponseType(type) {
    switch (type) {
      case "basic":
      case "cors":
      case "default":
        return true;
    }
    return false;
  }
}

function initSync(module) {
  if (wasm !== undefined) return wasm;

  if (module !== undefined) {
    if (Object.getPrototypeOf(module) === Object.prototype) {
      ({ module } = module);
    } else {
      console.warn("using deprecated parameters for `initSync()`; pass a single object instead");
    }
  }

  const imports = __wbg_get_imports();
  if (!(module instanceof WebAssembly.Module)) {
    module = new WebAssembly.Module(module);
  }
  const instance = new WebAssembly.Instance(module, imports);
  return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
  if (wasm !== undefined) return wasm;

  if (module_or_path !== undefined) {
    if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
      ({ module_or_path } = module_or_path);
    } else {
      console.warn(
        "using deprecated parameters for the initialization function; pass a single object instead",
      );
    }
  }

  if (module_or_path === undefined) {
    module_or_path = new URL("avoidant_wasm_bg.wasm", import.meta.url);
  }
  const imports = __wbg_get_imports();

  if (
    typeof module_or_path === "string" ||
    (typeof Request === "function" && module_or_path instanceof Request) ||
    (typeof URL === "function" && module_or_path instanceof URL)
  ) {
    module_or_path = fetch(module_or_path);
  }

  const { instance, module } = await __wbg_load(await module_or_path, imports);

  return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
