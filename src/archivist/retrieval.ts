/**
 * Archivist Retrieval
 *
 * Query helpers and formatters for retrieving lessons from the Archivist store.
 * Used by FM coding prompts to inject relevant lessons.
 */

import { Effect } from "effect";
import { LessonStore, makeLessonStoreLive } from "./store.js";
import type { ArchivistLesson } from "./schema.js";

// --- Query Types ---

/**
 * Query parameters for finding relevant lessons.
 */
export interface LessonQuery {
  /** Filter by source (terminal-bench, mechacoder, manual) */
  source?: ArchivistLesson["source"];
  /** Filter by model */
  model?: string;
  /** Filter by task labels/patterns in content */
  taskLabels?: string[];
  /** Filter by error patterns mentioned */
  errorPatterns?: string[];
  /** Maximum lessons to return */
  limit?: number;
  /** Minimum confidence threshold */
  minConfidence?: number;
}

// --- Retrieval Functions ---

/**
 * Get lessons relevant to a task description.
 * Sorts by confidence and recency.
 */
export const getRelevantLessons = (
  query: LessonQuery,
  projectRoot = process.cwd(),
): Effect.Effect<ArchivistLesson[], never, never> =>
  Effect.gen(function* () {
    const store = yield* LessonStore;
    let lessons = yield* store.getAll();

    // Filter by source if specified
    if (query.source) {
      lessons = lessons.filter((l) => l.source === query.source);
    }

    // Filter by model if specified
    if (query.model) {
      lessons = lessons.filter((l) => l.model === query.model);
    }

    // Filter by minimum confidence
    if (query.minConfidence !== undefined) {
      lessons = lessons.filter((l) => l.confidence >= query.minConfidence!);
    }

    // Filter by error patterns (keyword matching)
    if (query.errorPatterns?.length) {
      lessons = lessons.filter((l) =>
        l.failurePatterns?.some((fp) =>
          query.errorPatterns!.some((ep) =>
            fp.toLowerCase().includes(ep.toLowerCase()),
          ),
        ),
      );
    }

    // Filter by task labels (keyword matching in summary and tags)
    if (query.taskLabels?.length) {
      lessons = lessons.filter((l) =>
        query.taskLabels!.some(
          (label) =>
            l.summary.toLowerCase().includes(label.toLowerCase()) ||
            l.tags.some((t) => t.toLowerCase().includes(label.toLowerCase())),
        ),
      );
    }

    // Sort by confidence (descending) then recency
    lessons.sort((a, b) => {
      const confDiff = b.confidence - a.confidence;
      if (Math.abs(confDiff) > 0.1) return confDiff;
      return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
    });

    // Apply limit
    const limit = query.limit ?? 3;
    return lessons.slice(0, limit);
  }).pipe(
    Effect.provide(makeLessonStoreLive(projectRoot)),
    Effect.catchAll(() => Effect.succeed([] as ArchivistLesson[])),
  );

/**
 * Get lessons matching a task description using simple keyword matching.
 * Useful for finding lessons relevant to a specific coding task.
 */
export const getLessonsForTask = (
  taskDescription: string,
  options: {
    projectRoot?: string;
    limit?: number;
    source?: ArchivistLesson["source"];
    model?: string;
  } = {},
): Effect.Effect<ArchivistLesson[], never, never> => {
  // Extract keywords from task description
  const keywords = taskDescription
    .toLowerCase()
    .split(/\s+/)
    .filter((w) => w.length > 3)
    .slice(0, 10);

  return getRelevantLessons(
    {
      ...(options.source ? { source: options.source } : {}),
      ...(options.model ? { model: options.model } : {}),
      taskLabels: keywords,
      limit: options.limit ?? 3,
    },
    options.projectRoot,
  );
};

// --- Formatting Functions ---

/**
 * Format lessons for injection into FM prompts.
 * Produces a concise, readable format suitable for LLM context.
 */
export const formatLessonsForPrompt = (lessons: ArchivistLesson[]): string => {
  if (lessons.length === 0) {
    return "";
  }

  const formatted = lessons
    .map((l) => {
      let line = `- ${l.summary}`;
      if (l.skillsMentioned?.length) {
        line += ` (Skills: ${l.skillsMentioned.join(", ")})`;
      }
      return line;
    })
    .join("\n");

  return `Lessons from similar work:\n${formatted}`;
};

/**
 * Format lessons more concisely for FM's limited context.
 * Max ~200 chars per lesson.
 */
export const formatLessonsCompact = (
  lessons: ArchivistLesson[],
  maxCharsPerLesson = 200,
): string => {
  if (lessons.length === 0) {
    return "";
  }

  const formatted = lessons
    .map((l, i) => {
      const summary =
        l.summary.length > maxCharsPerLesson
          ? l.summary.slice(0, maxCharsPerLesson - 3) + "..."
          : l.summary;
      return `${i + 1}. ${summary}`;
    })
    .join("\n");

  return `Previous lessons:\n${formatted}`;
};

/**
 * Get and format lessons in one call.
 * Convenience function for FM prompt injection.
 */
export const getFormattedLessons = (
  query: LessonQuery,
  projectRoot = process.cwd(),
  compact = true,
): Effect.Effect<string, never, never> =>
  Effect.gen(function* () {
    const lessons = yield* getRelevantLessons(query, projectRoot);
    return compact
      ? formatLessonsCompact(lessons)
      : formatLessonsForPrompt(lessons);
  });
