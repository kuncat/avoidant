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
