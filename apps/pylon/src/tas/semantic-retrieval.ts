export type SemanticRetrievalItem<Ref extends string = string> = {
  readonly ref: Ref
  readonly embedding: readonly number[]
}

export function cosineSimilarity(
  a: readonly number[],
  b: readonly number[],
): number {
  if (a.length !== b.length) {
    throw new Error(
      `Embedding length mismatch: expected ${a.length}, received ${b.length}`,
    )
  }

  let dotProduct = 0
  let aMagnitudeSquared = 0
  let bMagnitudeSquared = 0

  for (let index = 0; index < a.length; index += 1) {
    const aValue = a[index]
    const bValue = b[index]

    dotProduct += aValue * bValue
    aMagnitudeSquared += aValue * aValue
    bMagnitudeSquared += bValue * bValue
  }

  if (aMagnitudeSquared === 0 || bMagnitudeSquared === 0) {
    return 0
  }

  return dotProduct / Math.sqrt(aMagnitudeSquared * bMagnitudeSquared)
}

export function topK<Ref extends string>(
  query: readonly number[],
  items: readonly SemanticRetrievalItem<Ref>[],
  k: number,
): Array<SemanticRetrievalItem<Ref>> {
  if (k <= 0) {
    return []
  }

  return items
    .map((item, index) => ({
      item,
      index,
      score: cosineSimilarity(query, item.embedding),
    }))
    .sort((left, right) => {
      if (right.score !== left.score) {
        return right.score - left.score
      }

      if (left.item.ref < right.item.ref) {
        return -1
      }

      if (left.item.ref > right.item.ref) {
        return 1
      }

      return left.index - right.index
    })
    .slice(0, Math.trunc(k))
    .map((ranked) => ranked.item)
}
