export { default as favicon } from "./assets/favicon.svg";
export { default as AvoidantGame } from "./components/AvoidantGame.svelte";

export type { CompetitiveConfig } from "./multiplayer/types";

export { default as AvoidantSpectator } from "./components/MultiplayerGame.svelte";
export type { SpectatorSession } from "./multiplayer/types";
export { currentMatch, forgetMatch } from "./multiplayer/current-match";
