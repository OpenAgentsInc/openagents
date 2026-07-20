/**
 * Neutral recall, ranking, and packing primitives.
 *
 * These reimplement the reviewed algorithm and tie-break ideas from the unwired
 * Pylon TAS kit (`semantic-retrieval.ts`, `session-memory.ts`,
 * `context-assembly.ts`). Per the AFS-10 packet, only the reviewed algorithm and
 * test ideas are reused; the TAS files carry no schema, persistence, consent,
 * delete, or owner-scope authority, so nothing is imported from them. Every
 * function here is pure and deterministic: equal inputs always give equal order,
 * so a recalled slice is auditable and stable.
 */

/** Cosine similarity of two equal-length vectors. Returns 0 for a zero vector. */
export const cosineSimilarity = (a: ReadonlyArray<number>, b: ReadonlyArray<number>): number => {
  if (a.length !== b.length) {
    throw new Error(`Embedding length mismatch: expected ${a.length}, received ${b.length}`);
  }
  let dot = 0;
  let aSq = 0;
  let bSq = 0;
  for (let index = 0; index < a.length; index += 1) {
    const av = a[index] ?? 0;
    const bv = b[index] ?? 0;
    dot += av * bv;
    aSq += av * av;
    bSq += bv * bv;
  }
  if (aSq === 0 || bSq === 0) return 0;
  return dot / Math.sqrt(aSq * bSq);
};

export type RankableItem<Ref extends string = string> = Readonly<{
  ref: Ref;
  embedding: ReadonlyArray<number>;
}>;

/** Top-k by cosine similarity, tie-broken by ref then original index for stability. */
export const topK = <Ref extends string>(
  query: ReadonlyArray<number>,
  items: ReadonlyArray<RankableItem<Ref>>,
  k: number,
): ReadonlyArray<RankableItem<Ref>> => {
  if (k <= 0) return [];
  return items
    .map((item, index) => ({ item, index, score: cosineSimilarity(query, item.embedding) }))
    .sort((left, right) => {
      if (right.score !== left.score) return right.score - left.score;
      if (left.item.ref < right.item.ref) return -1;
      if (left.item.ref > right.item.ref) return 1;
      return left.index - right.index;
    })
    .slice(0, Math.trunc(k))
    .map((ranked) => ranked.item);
};

export type SalienceItem<Ref extends string = string> = Readonly<{
  ref: Ref;
  salience: number;
  lastUsedAt: number;
}>;

/** Recall order without embeddings: salience plus a recency term, stable tie-break. */
export const recallOrderBySalience = <Ref extends string>(
  items: ReadonlyArray<SalienceItem<Ref>>,
  nowMs: number,
): ReadonlyArray<Ref> => {
  const recency = (item: SalienceItem<Ref>): number => 1 / (1 + Math.max(0, nowMs - item.lastUsedAt));
  return items
    .map((item, index) => ({ item, index, score: item.salience + recency(item) }))
    .sort((left, right) => {
      if (right.score !== left.score) return right.score - left.score;
      return left.index - right.index;
    })
    .map((ranked) => ranked.item.ref);
};

export type PackableItem<Ref extends string = string> = Readonly<{
  ref: Ref;
  priority: number;
  tokens: number;
  pinned?: boolean;
}>;

export type PackResult<Ref extends string = string> = Readonly<{
  included: ReadonlyArray<Ref>;
  dropped: ReadonlyArray<Ref>;
  usedTokens: number;
}>;

/** Token-budgeted packing: pinned items always fit first, then priority order. */
export const packWithinBudget = <Ref extends string>(
  items: ReadonlyArray<PackableItem<Ref>>,
  budgetTokens: number,
): PackResult<Ref> => {
  const pinned = items.filter((item) => item.pinned === true);
  const candidates = items
    .filter((item) => item.pinned !== true)
    .slice()
    .sort((left, right) => {
      if (right.priority !== left.priority) return right.priority - left.priority;
      return String(left.ref).localeCompare(String(right.ref));
    });
  const included: Ref[] = pinned.map((item) => item.ref);
  const dropped: Ref[] = [];
  let usedTokens = pinned.reduce((total, item) => total + item.tokens, 0);
  for (const item of candidates) {
    if (usedTokens + item.tokens <= budgetTokens) {
      included.push(item.ref);
      usedTokens += item.tokens;
    } else {
      dropped.push(item.ref);
    }
  }
  return { included, dropped, usedTokens };
};

/** A coarse, deterministic token estimate used only for budgeting the slice. */
export const estimateTokens = (text: string): number => Math.ceil(text.length / 4);
