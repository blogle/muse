/** Domain-agnostic raw field/network inspection and provenance presentation. */

export type RawFieldKind = 'scalar' | 'vector' | 'bool' | 'category' | 'index';
export type VectorRenderMode = 'direction' | 'magnitude' | 'direction_and_magnitude';

export interface FieldDisplayMetadata {
  name?: unknown;
  description?: unknown;
  units?: unknown;
  palette?: unknown;
  zeroMeaning?: unknown;
  renderRole?: unknown;
  vectorRenderMode?: unknown;
}

export interface NodeProvenance {
  nodeId?: unknown;
  operatorId?: unknown;
  operatorKind?: unknown;
  inputNodeIds?: unknown;
  inputReferenceIds?: unknown;
  arguments?: unknown;
  parameters?: unknown;
  outputBinding?: unknown;
}

export interface RawField {
  id: string;
  kind: RawFieldKind;
  values: unknown;
  display?: FieldDisplayMetadata | null;
  provenance?: NodeProvenance | null;
}

export interface RawNetwork {
  id: string;
  edges: unknown;
  values?: unknown;
  display?: FieldDisplayMetadata | null;
  provenance?: NodeProvenance | null;
}

export interface LegendEntry { label: string; value: string | number | boolean; count?: number }
export interface RawInspectorModel {
  id: string;
  kind: RawFieldKind | 'network';
  display: {
    name: string;
    description?: string;
    units?: string;
    palette?: string;
    zeroMeaning?: string;
    renderRole?: string;
    vectorRenderMode?: VectorRenderMode;
  };
  value: string;
  legend: LegendEntry[];
  provenance?: {
    nodeId?: string;
    operatorId?: string;
    operatorKind?: string;
    upstreamNodeIds: string[];
    inputReferenceIds: string[];
    arguments?: unknown;
    parameters?: unknown;
    outputBinding?: string;
  };
}

const text = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim().length > 0 ? value : undefined;

function displayModel(id: string, metadata?: FieldDisplayMetadata | null): RawInspectorModel['display'] {
  const mode = metadata?.vectorRenderMode;
  return {
    name: text(metadata?.name) ?? id,
    ...(text(metadata?.description) ? { description: text(metadata?.description) } : {}),
    ...(text(metadata?.units) ? { units: text(metadata?.units) } : {}),
    ...(text(metadata?.palette) ? { palette: text(metadata?.palette) } : {}),
    ...(text(metadata?.zeroMeaning) ? { zeroMeaning: text(metadata?.zeroMeaning) } : {}),
    ...(text(metadata?.renderRole) ? { renderRole: text(metadata?.renderRole) } : {}),
    ...(mode === 'direction' || mode === 'magnitude' || mode === 'direction_and_magnitude'
      ? { vectorRenderMode: mode } : {}),
  };
}

function upstreamIds(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === 'string') : [];
}

function provenanceModel(source?: NodeProvenance | null): RawInspectorModel['provenance'] {
  if (!source || typeof source !== 'object') return undefined;
  const nodeId = text(source.nodeId);
  const operatorId = text(source.operatorId);
  const operatorKind = text(source.operatorKind);
  const outputBinding = text(source.outputBinding);
  return {
    ...(nodeId ? { nodeId } : {}),
    ...(operatorId ? { operatorId } : {}),
    ...(operatorKind ? { operatorKind } : {}),
    upstreamNodeIds: upstreamIds(source.inputNodeIds),
    inputReferenceIds: upstreamIds(source.inputReferenceIds),
    ...(source.arguments !== undefined ? { arguments: source.arguments } : {}),
    ...(source.parameters !== undefined ? { parameters: source.parameters } : {}),
    ...(outputBinding ? { outputBinding } : {}),
  };
}

const validNumber = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);

function valueText(kind: RawFieldKind, value: unknown): string {
  if (kind === 'vector') {
    if (Array.isArray(value) && value.length >= 3 && value.slice(0, 3).every(validNumber))
      return `[${value.slice(0, 3).join(', ')}]`;
    if (value && typeof value === 'object' && ['x', 'y', 'z'].every((key) => validNumber((value as Record<string, unknown>)[key]))) {
      const vector = value as Record<string, number>;
      return `[${vector.x}, ${vector.y}, ${vector.z}]`;
    }
    return 'Unavailable';
  }
  if (kind === 'scalar') return validNumber(value) ? String(value) : 'Unavailable';
  if (kind === 'bool') return typeof value === 'boolean' ? String(value) : 'Unavailable';
  if ((kind === 'category' || kind === 'index') && (typeof value === 'string' || validNumber(value))) return String(value);
  return 'Unavailable';
}

function numericLegend(values: unknown): LegendEntry[] {
  if (!Array.isArray(values)) return [];
  const numbers = values.filter(validNumber);
  if (numbers.length === 0) return [];
  let min = numbers[0];
  let max = numbers[0];
  for (const number of numbers.slice(1)) {
    if (number < min) min = number;
    if (number > max) max = number;
  }
  return [{ label: 'Minimum', value: min }, { label: 'Maximum', value: max }];
}

function discreteLegend(values: unknown, kind: 'bool' | 'category' | 'index'): LegendEntry[] {
  if (!Array.isArray(values)) return [];
  const counts = new Map<string, { value: string | number | boolean; count: number }>();
  for (const value of values) {
    const valid = kind === 'bool' ? typeof value === 'boolean'
      : typeof value === 'string' || validNumber(value);
    if (!valid) continue;
    const key = `${typeof value}:${String(value)}`;
    const entry = counts.get(key);
    if (entry) entry.count += 1;
    else counts.set(key, { value, count: 1 });
  }
  return [...counts.values()].sort((a, b) => {
    const left = String(a.value);
    const right = String(b.value);
    return left < right ? -1 : left > right ? 1 : 0;
  })
    .map(({ value, count }) => ({ label: String(value), value, count }));
}

export function inspectRawField(field: RawField, index: number): RawInspectorModel {
  const values = Array.isArray(field.values) ? field.values : [];
  const isVector = field.kind === 'vector';
  const value = values[index];
  const discrete = field.kind === 'bool' || field.kind === 'category' || field.kind === 'index';
  return {
    id: field.id,
    kind: field.kind,
    display: displayModel(field.id, field.display),
    value: valueText(field.kind, value),
    legend: discrete ? discreteLegend(values, field.kind as 'bool' | 'category' | 'index')
      : numericLegend(isVector ? values.flatMap((entry) => Array.isArray(entry) ? entry : []) : values),
    ...(provenanceModel(field.provenance) ? { provenance: provenanceModel(field.provenance) } : {}),
  };
}

export function inspectRawNetwork(network: RawNetwork): RawInspectorModel {
  const edges = Array.isArray(network.edges) ? network.edges : [];
  const values = Array.isArray(network.values) ? network.values.filter(validNumber) : [];
  return {
    id: network.id,
    kind: 'network',
    display: displayModel(network.id, network.display),
    value: `${edges.length} edges${values.length ? `; ${values.length} edge values` : ''}`,
    legend: numericLegend(values),
    ...(provenanceModel(network.provenance) ? { provenance: provenanceModel(network.provenance) } : {}),
  };
}

/** Stable named integration point used by the prewired Wave 3 viewer barrel. */
export const wave3RawInspector = {
  inspectField: inspectRawField,
  inspectNetwork: inspectRawNetwork,
} as const;
