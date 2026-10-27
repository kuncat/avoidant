/**
 * The game this browser last joined. Scanning a TV's QR code opens a new tab, which has
 * no session storage, so the TV pairing page reads this to offer the game in progress.
 * It is kept per account: another account on the same browser must not join that game.
 */
const KEY = "avoidant:currentMatch";
const MAX_AGE = 12 * 3600_000;
export function rememberMatch(id: string, actor: string) {
  try {
    localStorage.setItem(KEY, JSON.stringify({ id, actor, at: Date.now() }));
  } catch {
    /* Storage can be unavailable in private windows. */
  }
}
export function forgetMatch() {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* Nothing was stored. */
  }
}
export function currentMatch(actor: string | undefined): string | null {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "null");
    return typeof saved?.id === "string" &&
      actor &&
      saved.actor === actor &&
      Date.now() - saved.at < MAX_AGE
      ? saved.id
      : null;
  } catch {
    return null;
  }
}
