/** Generic render-channel selection from serialized field display metadata. */

export const RENDER_CHANNELS = [
  'geometry_displacement',
  'base_material',
  'material_override',
  'line_overlay',
  'scalar_overlay',
  'vector_overlay',
] as const;

export type RenderChannel = (typeof RENDER_CHANNELS)[number];

export type FieldMetadata = {
  render_role?: unknown;
  display_name?: unknown;
  recommended_palette?: unknown;
  vector_render_mode?: unknown;
};

export type RenderChannelBinding = {
  channel: RenderChannel;
  field: string;
  displayName: string;
  palette?: string;
  vectorRenderMode?: 'direction_only' | 'direction_and_magnitude';
};

export type RenderChannelMetadata = Record<string, FieldMetadata | null | undefined>;

/** Priority from bottom-most/default rendering to top-most overlays. */
export const RENDER_CHANNEL_ORDER: readonly RenderChannel[] = RENDER_CHANNELS;

/**
 * Bind valid metadata in stable channel and field-name order. When a channel
 * has several candidates, the lexically first field is selected. A material
 * override takes precedence over the base material in resolveMaterialBinding.
 */
export function bindRenderChannels(metadata: RenderChannelMetadata | null | undefined): RenderChannelBinding[] {
  if (!metadata || typeof metadata !== 'object' || Array.isArray(metadata)) return [];

  const names = Object.keys(metadata).sort(compareNames);
  const bindings: RenderChannelBinding[] = [];
  for (const channel of RENDER_CHANNEL_ORDER) {
    for (const field of names) {
      const value = metadata[field];
      if (!value || typeof value !== 'object' || Array.isArray(value) || value.render_role !== channel) continue;
      const binding: RenderChannelBinding = {
        channel,
        field,
        displayName: typeof value.display_name === 'string' && value.display_name.trim() ? value.display_name : field,
      };
      if (typeof value.recommended_palette === 'string' && value.recommended_palette.trim()) {
        binding.palette = value.recommended_palette;
      }
      if (value.vector_render_mode === 'direction_only' || value.vector_render_mode === 'direction_and_magnitude') {
        binding.vectorRenderMode = value.vector_render_mode;
      }
      bindings.push(binding);
      break;
    }
  }
  return bindings;
}

/** Material overlays replace the base material when present. */
export function resolveMaterialBinding(bindings: readonly RenderChannelBinding[]): RenderChannelBinding | undefined {
  return bindings.find((binding) => binding.channel === 'material_override')
    ?? bindings.find((binding) => binding.channel === 'base_material');
}

function compareNames(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

/** Kept as a compatibility marker for existing Wave 3 barrel exports. */
export const wave3RenderChannels = 'metadata-driven' as const;
