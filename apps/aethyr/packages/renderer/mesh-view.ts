/** Exact triangle edges for inspection; no project or collision data changes. */
export function triangleEdges(indices: Uint32Array): Uint32Array {
  if (indices.length % 3 !== 0 || indices.byteLength > 16 * 1024 * 1024)
    throw new Error("This mesh is too large for the inspection overlay.");
  const edges = new Uint32Array(indices.length * 2);
  for (let i = 0, j = 0; i < indices.length; i += 3, j += 6) {
    const a = indices[i],
      b = indices[i + 1],
      c = indices[i + 2];
    edges.set([a, b, b, c, c, a], j);
  }
  return edges;
}
export const MESH_FRAGMENT = `
@fragment fn meshFS(v:Out)->@location(0) vec4<f32> {
  return vec4(0.08,1.4,1.7,1.0);
}`;
