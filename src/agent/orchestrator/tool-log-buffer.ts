export interface ToolLogBuffer {
  id: string;
  name?: string;
  chunks: string[];
}

export type ToolLogBufferMap = Map<string, ToolLogBuffer>;

export const ensureToolBuffer = (
  buffers: ToolLogBufferMap,
  id: string,
  name?: string,
): ToolLogBuffer => {
  const existing = buffers.get(id);
  if (existing) {
    if (name && !existing.name) existing.name = name;
    return existing;
  }
  const created: ToolLogBuffer = { id, chunks: [] };
  if (name) created.name = name;
  buffers.set(id, created);
  return created;
};

export const appendToolChunk = (
  buffers: ToolLogBufferMap,
  id: string,
  chunk: string | undefined,
): void => {
  if (!chunk) return;
  const buffer = ensureToolBuffer(buffers, id);
  buffer.chunks.push(chunk);
};

const parseInput = (raw: string | undefined): unknown => {
  if (!raw) return undefined;
  try {
    return JSON.parse(raw);
  } catch {
    return raw;
  }
};

export const buildToolPayload = (
  buffers: ToolLogBufferMap,
  id: string,
  finalInput?: unknown,
): { tool?: string; id: string; input?: unknown } => {
  const buffer = buffers.get(id);
  const raw = buffer?.chunks.join("") ?? undefined;
  const parsed = parseInput(raw);
  const input = finalInput !== undefined ? finalInput : parsed;
  const payload: { id: string; tool?: string; input?: unknown } = { id };
  if (buffer?.name) payload.tool = buffer.name;
  if (input !== undefined) payload.input = input;
  return payload;
};
