/** Generic, validated view models for causal vector, scalar, and network overlays. */

export type VectorDisplayMode = 'direction_only' | 'direction_and_magnitude';

export interface VectorOverlayItem {
  cellId: number;
  direction: [number, number, number];
  magnitude: number;
}

export interface VectorOverlayModel {
  kind: 'vector';
  mode: VectorDisplayMode;
  disclosure: string;
  magnitudeEncoding: 'separate' | 'absent';
  items: VectorOverlayItem[];
}

export interface ScalarOverlayModel {
  kind: 'scalar';
  legend: { min: number; max: number; count: number } | null;
  values: { cellId: number; value: number }[];
}

export interface NetworkOverlayModel {
  kind: 'network';
  edges: { source: number; target: number; value?: number }[];
}

export interface OverlaySelection<T> {
  options: string[];
  selected: string | null;
  value: T | null;
}

const finiteTriple = (value: unknown): value is [number, number, number] =>
  Array.isArray(value) && value.length === 3 && value.every(Number.isFinite);

/** Sort available overlay names and resolve selection independently of input key order. */
export function selectOverlay<T>(
  overlays: Record<string, T> | null | undefined,
  requested?: string | null,
): OverlaySelection<T> {
  const options = Object.keys(overlays ?? {}).sort((a, b) => a < b ? -1 : a > b ? 1 : 0);
  const selected = requested && options.includes(requested) ? requested : options[0] ?? null;
  return { options, selected, value: selected === null ? null : overlays?.[selected] ?? null };
}

/** Directions are normalized for arrows; magnitude remains a separate scalar channel. */
export function buildVectorOverlay(
  raw: unknown,
  mode: VectorDisplayMode = 'direction_and_magnitude',
): VectorOverlayModel {
  const rows = Array.isArray(raw) ? raw : [];
  const items: VectorOverlayItem[] = [];
  rows.forEach((candidate, cellId) => {
    if (!finiteTriple(candidate)) return;
    const [x, y, z] = candidate;
    const magnitude = Math.hypot(x, y, z);
    if (!Number.isFinite(magnitude)) return;
    const direction: [number, number, number] = magnitude === 0
      ? [0, 0, 0]
      : [x / magnitude, y / magnitude, z / magnitude];
    items.push({ cellId, direction, magnitude });
  });
  const directionOnly = mode === 'direction_only';
  return {
    kind: 'vector',
    mode: directionOnly ? 'direction_only' : 'direction_and_magnitude',
    disclosure: directionOnly
      ? 'Direction only; vector magnitudes are available separately but are not encoded.'
      : 'Arrow direction is normalized; original magnitude is preserved as a separate scalar value.',
    magnitudeEncoding: directionOnly ? 'absent' : 'separate',
    items,
  };
}

/** Produce stable finite scalar samples and legend bounds; malformed entries are skipped. */
export function buildScalarOverlay(raw: unknown): ScalarOverlayModel {
  const values: { cellId: number; value: number }[] = [];
  if (Array.isArray(raw)) {
    raw.forEach((value: unknown, cellId) => {
      if (typeof value === 'number' && Number.isFinite(value)) values.push({ cellId, value });
    });
  }
  let min = Infinity;
  let max = -Infinity;
  for (const { value } of values) {
    min = Math.min(min, value);
    max = Math.max(max, value);
  }
  return {
    kind: 'scalar',
    legend: values.length === 0 ? null : { min, max, count: values.length },
    values,
  };
}

/** Validate endpoint references and optional edge values, then order edges canonically. */
export function buildNetworkOverlay(
  raw: unknown,
  cellCount: number,
): NetworkOverlayModel {
  const edges: NetworkOverlayModel['edges'] = [];
  if (!Number.isSafeInteger(cellCount) || cellCount < 0 || !raw || typeof raw !== 'object') {
    return { kind: 'network', edges };
  }
  const network = raw as { edges?: unknown; values?: unknown };
  if (!Array.isArray(network.edges)) return { kind: 'network', edges };
  const values = Array.isArray(network.values) ? network.values : undefined;
  network.edges.forEach((candidate, index) => {
    if (!Array.isArray(candidate) || candidate.length !== 2) return;
    const [source, target] = candidate;
    if (!Number.isSafeInteger(source) || !Number.isSafeInteger(target)
      || source < 0 || target < 0 || source >= cellCount || target >= cellCount) return;
    const edge: NetworkOverlayModel['edges'][number] = { source, target };
    const value = values?.[index];
    if (typeof value === 'number' && Number.isFinite(value)) edge.value = value;
    edges.push(edge);
  });
  edges.sort((a, b) => a.source - b.source || a.target - b.target
    || (a.value ?? 0) - (b.value ?? 0));
  return { kind: 'network', edges };
}

/** Stable module surface already consumed by the Wave 3 viewer registration. */
export const wave3CausalOverlays = {
  select: selectOverlay,
  vector: buildVectorOverlay,
  scalar: buildScalarOverlay,
  network: buildNetworkOverlay,
} as const;
