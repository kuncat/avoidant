# Avoidant

## Developing

### Prerequisites

WebAssembly compilation target

```sh
rustup target add wasm32-unknown-unknown
```

wasm-pack

```sh
cargo install wasm-pack
```

Nightly Rust toolchain (required for WebAssembly threads via wasm-bindgen-rayon)

```sh
rustup toolchain install nightly --component rust-src --target wasm32-unknown-unknown
```

On static hosting, ensure cross-origin isolation headers (for browser WebAssembly threads):
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`

### Running

Install dependencies with `npm install` then start a development server:

```sh
npm run dev

# or start the server and open the app in a new browser tab
npm run dev -- --open
```

## Building

### WASM

```
npm run build:wasm
```

### App

```sh
npm run build
```

You can preview the production build with `npm run preview`.

## Testing

### WASM Headless Browser test

```
cd crates/game && wasm-pack test --headless --firefox
```

## Multiplayer synchronization

The player who creates the game is its authority and must remain online. Invitations
become available after the safe opening move. Invitees rebuild the same map and void
layout, then wait for the host's complete explored-cell and score snapshot before
playing. Create fresh invitations after updating from older protocol versions.

Moves carry request IDs. The host applies each request once and publishes a numbered
snapshot; peers ignore older snapshots. Requests and snapshots are retried every
three seconds, including after a reconnect. Reveal animations only update display
flags; they cannot change the canonical board, score, streak, or completion state.
Host migration and playing offline as a guest are not supported.

Native synchronization regression tests:

```sh
cargo test -p avoidant --lib
```

WASM integration test (Node.js, with the matching wasm-bindgen-test-runner on PATH):

```sh
NODE_PATH="$PWD/node_modules" CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test -p avoidant --lib --target wasm32-unknown-unknown
```
