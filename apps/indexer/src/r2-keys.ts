/**
 * R2 object key layout: raw/posts|comments|authors/{yyyy}/{mm}/{dd}/...
 * Quarantine: quarantine/{yyyy}/{mm}/{dd}/{kind}/{id}.json
 */

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

export function r2KeyPost(date: Date, postId: string): string {
  const y = date.getUTCFullYear();
  const m = date.getUTCMonth() + 1;
  const d = date.getUTCDate();
  return `raw/posts/${y}/${pad(m)}/${pad(d)}/${postId}.json`;
}

export function r2KeyCommentsPage(date: Date, postId: string, pageOrCursor: string): string {
  const y = date.getUTCFullYear();
  const m = date.getUTCMonth() + 1;
  const d = date.getUTCDate();
  return `raw/comments/${y}/${pad(m)}/${pad(d)}/${postId}/${pageOrCursor}.json`;
}

export function r2KeyAuthor(date: Date, authorName: string): string {
  const y = date.getUTCFullYear();
  const m = date.getUTCMonth() + 1;
  const d = date.getUTCDate();
  const safe = authorName.replace(/[^a-zA-Z0-9_-]/g, '_').slice(0, 128);
  return `raw/authors/${y}/${pad(m)}/${pad(d)}/${safe}.json`;
}

export function r2KeyQuarantine(date: Date, kind: string, id: string): string {
  const y = date.getUTCFullYear();
  const m = date.getUTCMonth() + 1;
  const d = date.getUTCDate();
  return `quarantine/${y}/${pad(m)}/${pad(d)}/${kind}/${id}.json`;
}
