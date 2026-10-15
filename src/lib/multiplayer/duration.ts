export function formatDuration(milliseconds: number, locale: string): string {
  const seconds = Math.floor(Math.max(0, milliseconds) / 1000);
  return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60]
    .map((value) => value.toLocaleString(locale, { minimumIntegerDigits: 2, useGrouping: false }))
    .join(":");
}
