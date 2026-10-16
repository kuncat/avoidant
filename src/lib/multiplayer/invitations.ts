/** Accept existing UUID invitations, share codes, and exact kuga.run links. */
export function parseInvitation(input: string): string | null {
  let value = input.trim();
  try {
    if (/^(?:https?:\/\/)?kuga\.run\//i.test(value)) {
      const url = new URL(/^https?:/i.test(value) ? value : `https://${value}`);
      if (url.hostname !== "kuga.run" || url.username || url.password || url.port) return null;
      value = url.pathname.slice(1);
    } else if (/^https?:\/\//i.test(value)) {
      value = new URL(value).searchParams.get("match") ?? "";
    }
  } catch {
    return null;
  }
  if (/^[A-Za-z]{4,32}$/.test(value)) return value.toUpperCase();
  return /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value)
    ? value
    : null;
}
