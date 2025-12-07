/**
 * Skill Embedding Service
 *
 * Generates embeddings for skills using Apple Foundation Models.
 * Uses cosine similarity for skill retrieval.
 */

import { Effect, Context, Layer, Ref } from "effect";
import { FMService } from "../fm/service.js";
import type { Skill } from "./schema.js";

// --- Configuration ---

const EMBEDDING_DIMENSION = 768; // Typical embedding dimension
const CACHE_MAX_SIZE = 1000; // Max cached embeddings

// --- Error Types ---

export class EmbeddingError extends Error {
  readonly _tag = "EmbeddingError";
  constructor(
    readonly reason: "generation_failed" | "invalid_response" | "cache_error",
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "EmbeddingError";
  }
}

// --- Embedding Service Interface ---

export interface IEmbeddingService {
  /** Generate embedding for text */
  readonly embed: (text: string) => Effect.Effect<number[], EmbeddingError>;

  /** Generate embedding for a skill (uses description + name + tags) */
  readonly embedSkill: (skill: Skill) => Effect.Effect<number[], EmbeddingError>;

  /** Compute cosine similarity between two embeddings */
  readonly similarity: (a: number[], b: number[]) => number;

  /** Find top-K similar skills from candidates */
  readonly findSimilar: (
    query: number[],
    candidates: Array<{ skill: Skill; embedding: number[] }>,
    topK: number,
    minSimilarity?: number,
  ) => Array<{ skill: Skill; similarity: number }>;

  /** Clear the embedding cache */
  readonly clearCache: () => Effect.Effect<void, never>;

  /** Get cache stats */
  readonly getCacheStats: () => Effect.Effect<{ size: number; hits: number; misses: number }, never>;
}

// --- Embedding Service Tag ---

export class EmbeddingService extends Context.Tag("EmbeddingService")<
  EmbeddingService,
  IEmbeddingService
>() {}

// --- Cache Types ---

interface CacheEntry {
  text: string;
  embedding: number[];
  timestamp: number;
}

interface CacheState {
  entries: Map<string, CacheEntry>;
  hits: number;
  misses: number;
}

// --- Implementation ---

/**
 * Compute cosine similarity between two vectors.
 */
export const cosineSimilarity = (a: number[], b: number[]): number => {
  if (a.length !== b.length) {
    throw new Error(`Vector dimension mismatch: ${a.length} vs ${b.length}`);
  }

  let dotProduct = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  if (normA === 0 || normB === 0) return 0;
  return dotProduct / (Math.sqrt(normA) * Math.sqrt(normB));
};

/**
 * Hash text for cache key.
 */
const hashText = (text: string): string => {
  let hash = 0;
  for (let i = 0; i < text.length; i++) {
    const char = text.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash;
  }
  return hash.toString(36);
};

/**
 * Build skill text for embedding.
 */
export const buildSkillText = (skill: Skill): string => {
  const parts = [
    skill.name,
    skill.description,
    skill.category,
    ...(skill.tags ?? []),
    ...(skill.languages ?? []),
    ...(skill.frameworks ?? []),
  ];
  return parts.filter(Boolean).join(" ");
};

/**
 * Create embedding service using FM for generation.
 *
 * Note: Since FM may not have native embedding support, we use a
 * text-based approach where we ask FM to extract semantic features
 * and then hash them into a pseudo-embedding.
 */
const makeEmbeddingService = (): Effect.Effect<
  IEmbeddingService,
  never,
  FMService
> =>
  Effect.gen(function* () {
    const fm = yield* FMService;
    const cacheRef = yield* Ref.make<CacheState>({
      entries: new Map(),
      hits: 0,
      misses: 0,
    });

    // Simple hash-based embedding (fallback when FM embeddings not available)
    // This uses a deterministic hash function to create pseudo-embeddings
    const hashEmbed = (text: string): number[] => {
      const normalized = text.toLowerCase().trim();
      const words = normalized.split(/\s+/).filter((w) => w.length > 0);
      const embedding = new Array(EMBEDDING_DIMENSION).fill(0);

      // Use word hashing with position weighting
      for (let i = 0; i < words.length; i++) {
        const word = words[i];
        const positionWeight = 1 / (1 + i * 0.1); // Earlier words have more weight

        for (let j = 0; j < word.length; j++) {
          const charCode = word.charCodeAt(j);
          const idx1 = (charCode * 31 + j) % EMBEDDING_DIMENSION;
          const idx2 = (charCode * 37 + j * 7) % EMBEDDING_DIMENSION;
          const idx3 = (charCode * 41 + j * 13) % EMBEDDING_DIMENSION;

          embedding[idx1] += positionWeight * 0.5;
          embedding[idx2] += positionWeight * 0.3;
          embedding[idx3] += positionWeight * 0.2;
        }
      }

      // Normalize to unit length
      let norm = 0;
      for (let i = 0; i < embedding.length; i++) {
        norm += embedding[i] * embedding[i];
      }
      norm = Math.sqrt(norm);

      if (norm > 0) {
        for (let i = 0; i < embedding.length; i++) {
          embedding[i] /= norm;
        }
      }

      return embedding;
    };

    const embed = (text: string): Effect.Effect<number[], EmbeddingError> =>
      Effect.gen(function* () {
        const key = hashText(text);

        // Check cache
        const cache = yield* Ref.get(cacheRef);
        const cached = cache.entries.get(key);

        if (cached) {
          yield* Ref.update(cacheRef, (s) => ({ ...s, hits: s.hits + 1 }));
          return cached.embedding;
        }

        yield* Ref.update(cacheRef, (s) => ({ ...s, misses: s.misses + 1 }));

        // Try to use FM for semantic extraction
        // Fall back to hash-based embedding if FM fails
        const embedding = yield* Effect.gen(function* () {
          try {
            // Ask FM to extract semantic keywords
            const response = yield* fm.chat({
              messages: [
                {
                  role: "system",
                  content:
                    "You are a semantic keyword extractor. Given text, output exactly 20 relevant keywords separated by commas. Only output the keywords, nothing else.",
                },
                {
                  role: "user",
                  content: text.slice(0, 1000), // Limit input size
                },
              ],
            }).pipe(
              Effect.timeout("10 seconds"),
              Effect.catchAll(() => Effect.succeed(null)),
            );

            if (response?.choices[0]?.message.content) {
              // Use keywords + original text for embedding
              const keywords = response.choices[0].message.content;
              return hashEmbed(`${text} ${keywords}`);
            }
          } catch {
            // FM failed, fall back to hash
          }

          // Fall back to hash-based embedding
          return hashEmbed(text);
        });

        // Cache the result
        yield* Ref.update(cacheRef, (s) => {
          const newEntries = new Map(s.entries);

          // Evict oldest entries if cache is full
          if (newEntries.size >= CACHE_MAX_SIZE) {
            const oldest = Array.from(newEntries.entries())
              .sort((a, b) => a[1].timestamp - b[1].timestamp)
              .slice(0, 100);
            for (const [k] of oldest) {
              newEntries.delete(k);
            }
          }

          newEntries.set(key, {
            text,
            embedding,
            timestamp: Date.now(),
          });

          return { ...s, entries: newEntries };
        });

        return embedding;
      });

    const embedSkill = (skill: Skill): Effect.Effect<number[], EmbeddingError> =>
      embed(buildSkillText(skill));

    const similarity = (a: number[], b: number[]): number => cosineSimilarity(a, b);

    const findSimilar = (
      query: number[],
      candidates: Array<{ skill: Skill; embedding: number[] }>,
      topK: number,
      minSimilarity = 0,
    ): Array<{ skill: Skill; similarity: number }> => {
      const scored = candidates
        .map((c) => ({
          skill: c.skill,
          similarity: cosineSimilarity(query, c.embedding),
        }))
        .filter((s) => s.similarity >= minSimilarity)
        .sort((a, b) => b.similarity - a.similarity);

      return scored.slice(0, topK);
    };

    const clearCache = (): Effect.Effect<void, never> =>
      Ref.set(cacheRef, { entries: new Map(), hits: 0, misses: 0 });

    const getCacheStats = (): Effect.Effect<
      { size: number; hits: number; misses: number },
      never
    > =>
      Ref.get(cacheRef).pipe(
        Effect.map((s) => ({
          size: s.entries.size,
          hits: s.hits,
          misses: s.misses,
        })),
      );

    return {
      embed,
      embedSkill,
      similarity,
      findSimilar,
      clearCache,
      getCacheStats,
    };
  });

// --- Layer ---

/**
 * EmbeddingService layer that requires FMService.
 */
export const EmbeddingServiceLive: Layer.Layer<EmbeddingService, never, FMService> =
  Layer.effect(EmbeddingService, makeEmbeddingService());

/**
 * Create EmbeddingService layer with custom FM layer.
 */
export const makeEmbeddingServiceLayer = (
  fmLayer: Layer.Layer<FMService, never, never>,
): Layer.Layer<EmbeddingService, never, never> =>
  Layer.provide(EmbeddingServiceLive, fmLayer);
