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

## Multiplayer

The existing New Game menu offers Solo, Collaborative and Territory. All
multiplayer modes require a Kuncat account and use the trusted Rust authority.
Collaborative games share reveals and a team score; territory use their
respective competitive rules. Invitations join through the same menu and login flow.
Solo stays local and does not require an account. Iroh transport and peer host/join APIs have been removed.

See [the implementation plan](docs/multiplayer-plan.md) for rules, identity and
architecture diagrams, and [the server guide](server/README.md) for deployment.
The component's optional `competitive` configuration supplies the server URL and
Kuncat account client for all multiplayer modes. Without configuration,
multiplayer controls remain disabled. The sibling website configures shared OIDC;
the sibling infra repository provisions identity, database and Workers.

```sh
npm ci
npm --prefix server ci
npm run dev:multiplayer -- --host
```

The local launcher supplies test accounts to the ordinary menu. Production builds
exclude that adapter. Open the printed player URLs, choose New Game and a mode,
then share the invitation link. No cloud provisioning is needed for local play.

```sh
cargo test -p avoidant --lib
cargo test -p avoidant-competitive
npm --prefix server test
```

The shared menu uses DaisyUI 5 with Tailwind 4. Consumers of the Svelte package
must install `daisyui` and enable its Tailwind plugin (themes `winter` and `night`),
as configured in `src/app.css` and the sibling games site's stylesheet.
