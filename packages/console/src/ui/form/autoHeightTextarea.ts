/** Compute CSS height for an auto-growing textarea from its scrollHeight. */
export function computeAutoHeight(scrollHeight: number, minPx: number, maxPx?: number): string {
  const clamped = Math.max(minPx, maxPx == null ? scrollHeight : Math.min(scrollHeight, maxPx));
  return `${clamped}px`;
}
