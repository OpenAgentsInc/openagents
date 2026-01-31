/**
 * Extract author and submolt from Moltbook API objects (nested or flat).
 */

function getObj(obj: unknown, key: string): unknown {
  if (obj && typeof obj === 'object' && key in obj) {
    return (obj as Record<string, unknown>)[key];
  }
  return undefined;
}

/** Author is often { name, id } on post/comment. */
export function getAuthorName(item: unknown): string | null {
  const author = getObj(item, 'author');
  if (author && typeof author === 'object') {
    const name = getObj(author, 'name');
    if (typeof name === 'string' && name.trim()) return name.trim();
  }
  const name = getObj(item, 'author_name');
  if (typeof name === 'string' && name.trim()) return name.trim();
  return null;
}

export function getAuthorId(item: unknown): string | null {
  const author = getObj(item, 'author');
  if (author && typeof author === 'object') {
    const id = getObj(author, 'id');
    if (typeof id === 'string' && id.trim()) return id.trim();
  }
  const id = getObj(item, 'author_id');
  if (typeof id === 'string' && id.trim()) return id.trim();
  return null;
}

/** Submolt can be string "general" or object { name, display_name, id }. */
export function getSubmoltName(submolt: unknown): string | null {
  if (typeof submolt === 'string' && submolt.trim()) return submolt.trim();
  if (submolt && typeof submolt === 'object') {
    const name = getObj(submolt, 'name');
    if (typeof name === 'string' && name.trim()) return name.trim();
    const display = getObj(submolt, 'display_name');
    if (typeof display === 'string' && display.trim()) return display.trim();
  }
  return null;
}
