# Avoidant

## Developing

### Prerequisites

wasm-pack

```sh
cargo install wasm-pack
```

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
