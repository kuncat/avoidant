// Stable, non-secret UUIDs for the explicit local multiplayer harness only.
// This also works on LAN HTTP origins where SubtleCrypto is unavailable.
export function localActorId(actor: string): string {
  const hex = [0, 1, 2, 3]
    .map((salt) => {
      let hash = 2166136261 ^ salt;
      for (const character of actor) hash = Math.imul(hash ^ character.codePointAt(0)!, 16777619);
      return (hash >>> 0).toString(16).padStart(8, "0");
    })
    .join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-8${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
}
