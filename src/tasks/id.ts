import { Effect } from "effect";

/**
 * Generate a deterministic content-based hash ID.
 * Format: prefix-{6-char-hex} with progressive extension on collision.
 *
 * The hash is computed from title, description, timestamp, and optional workspace ID.
 * Returns the full hash for progressive collision handling - caller extracts [:6], [:7], [:8] as needed.
 */
export const generateHashId = (
  prefix: string,
  title: string,
  description: string,
  createdAt: Date,
  workspaceId = "",
): Effect.Effect<string, never, never> =>
  Effect.sync(() => {
    const encoder = new TextEncoder();
    const data = encoder.encode(
      title + description + createdAt.toISOString() + workspaceId,
    );

    // Use Bun's native crypto for SHA-256
    const hasher = new Bun.CryptoHasher("sha256");
    hasher.update(data);
    const hash = hasher.digest("hex");

    return hash;
  });

/**
 * Generate a short ID with the given prefix.
 * Returns prefix-{6-char-hex} by default.
 */
export const generateShortId = (
  prefix: string,
  title: string,
  description = "",
  createdAt = new Date(),
  length = 6,
): Effect.Effect<string, never, never> =>
  generateHashId(prefix, title, description, createdAt).pipe(
    Effect.map((hash) => `${prefix}-${hash.slice(0, length)}`),
  );

/**
 * Generate a random short ID (for cases where content-based is not needed).
 * Uses crypto.randomUUID() for randomness.
 */
export const generateRandomId = (
  prefix: string,
  length = 6,
): Effect.Effect<string, never, never> =>
  Effect.sync(() => {
    const uuid = crypto.randomUUID().replace(/-/g, "");
    return `${prefix}-${uuid.slice(0, length)}`;
  });

/**
 * Generate a child ID in hierarchical format.
 * Format: parent.N (e.g., "oa-abc123.1", "oa-abc123.1.2")
 */
export const generateChildId = (
  parentId: string,
  childNumber: number,
): string => `${parentId}.${childNumber}`;

/**
 * Parse a hierarchical ID to extract root, parent, and depth.
 *
 * Examples:
 *   "oa-abc123" → { rootId: "oa-abc123", parentId: null, depth: 0 }
 *   "oa-abc123.1" → { rootId: "oa-abc123", parentId: "oa-abc123", depth: 1 }
 *   "oa-abc123.1.2" → { rootId: "oa-abc123", parentId: "oa-abc123.1", depth: 2 }
 */
export const parseHierarchicalId = (
  id: string,
): { rootId: string; parentId: string | null; depth: number } => {
  const parts = id.split(".");

  if (parts.length === 1) {
    return { rootId: id, parentId: null, depth: 0 };
  }

  const rootId = parts[0];
  const parentId = parts.slice(0, -1).join(".");
  const depth = parts.length - 1;

  return { rootId, parentId, depth };
};

/**
 * Check if a task ID matches or is a child of a given parent ID.
 */
export const isChildOf = (taskId: string, parentId: string): boolean => {
  return taskId.startsWith(parentId + ".");
};

/**
 * Get the immediate parent ID of a hierarchical ID.
 * Returns null if the ID has no parent.
 */
export const getParentId = (id: string): string | null => {
  const { parentId } = parseHierarchicalId(id);
  return parentId;
};

/**
 * Maximum hierarchy depth (prevents over-decomposition).
 */
export const MAX_HIERARCHY_DEPTH = 3;

/**
 * Check if an ID can have children (not already at max depth).
 */
export const canHaveChildren = (id: string): boolean => {
  const { depth } = parseHierarchicalId(id);
  return depth < MAX_HIERARCHY_DEPTH;
};

/**
 * Find the next available child number for a parent.
 * Examines existing IDs to find gaps or the next sequential number.
 */
export const findNextChildNumber = (
  parentId: string,
  existingIds: string[],
): number => {
  const childNumbers = existingIds
    .filter((id) => isChildOf(id, parentId))
    .map((id) => {
      const suffix = id.slice(parentId.length + 1);
      const firstPart = suffix.split(".")[0];
      return parseInt(firstPart, 10);
    })
    .filter((n) => !isNaN(n));

  if (childNumbers.length === 0) return 1;

  return Math.max(...childNumbers) + 1;
};
