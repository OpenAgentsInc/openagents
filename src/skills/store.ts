/**
 * Skill Library Store
 *
 * JSONL-based storage for skills with in-memory caching.
 * Location: .openagents/skills/library.jsonl
 */

import { Effect, Ref, Context, Layer } from "effect";
import * as S from "effect/Schema";
import { existsSync, mkdirSync, readFileSync, writeFileSync, appendFileSync } from "fs";
import { dirname, join } from "path";
import { type Skill, type SkillFilter, type SkillStatus, Skill as SkillSchema } from "./schema.js";

// --- Configuration ---

const DEFAULT_SKILLS_DIR = ".openagents/skills";
const LIBRARY_FILE = "library.jsonl";
const INDEX_FILE = "index.json";

// --- Error Types ---

export class SkillStoreError extends Error {
  readonly _tag = "SkillStoreError";
  constructor(
    readonly reason: "io_error" | "parse_error" | "not_found" | "duplicate",
    message: string,
  ) {
    super(message);
    this.name = "SkillStoreError";
  }
}

// --- Index Types ---

interface SkillIndex {
  version: number;
  count: number;
  byId: Record<string, number>; // id -> line number (0-indexed)
  byCategory: Record<string, string[]>; // category -> skill ids
  byTag: Record<string, string[]>; // tag -> skill ids
  lastUpdated: string;
}

const createEmptyIndex = (): SkillIndex => ({
  version: 1,
  count: 0,
  byId: {},
  byCategory: {},
  byTag: {},
  lastUpdated: new Date().toISOString(),
});

// --- Store Interface ---

export interface ISkillStore {
  /** Get a skill by ID */
  readonly get: (id: string) => Effect.Effect<Skill | null, SkillStoreError>;

  /** List all skills with optional filter */
  readonly list: (filter?: SkillFilter) => Effect.Effect<Skill[], SkillStoreError>;

  /** Add a new skill */
  readonly add: (skill: Skill) => Effect.Effect<void, SkillStoreError>;

  /** Update an existing skill */
  readonly update: (skill: Skill) => Effect.Effect<void, SkillStoreError>;

  /** Delete a skill (archives it) */
  readonly archive: (id: string) => Effect.Effect<void, SkillStoreError>;

  /** Get skills by category */
  readonly getByCategory: (category: string) => Effect.Effect<Skill[], SkillStoreError>;

  /** Get skills by tag */
  readonly getByTag: (tag: string) => Effect.Effect<Skill[], SkillStoreError>;

  /** Get skill count */
  readonly count: () => Effect.Effect<number, never>;

  /** Reload from disk */
  readonly reload: () => Effect.Effect<void, SkillStoreError>;

  /** Get the store path */
  readonly getPath: () => string;
}

// --- Store Tag ---

export class SkillStore extends Context.Tag("SkillStore")<SkillStore, ISkillStore>() {}

// --- Store State ---

interface StoreState {
  skills: Map<string, Skill>;
  index: SkillIndex;
  dirty: boolean;
}

// --- Store Implementation ---

const makeStore = (projectRoot: string): Effect.Effect<ISkillStore, SkillStoreError> =>
  Effect.gen(function* () {
    const skillsDir = join(projectRoot, DEFAULT_SKILLS_DIR);
    const libraryPath = join(skillsDir, LIBRARY_FILE);
    const indexPath = join(skillsDir, INDEX_FILE);

    // Ensure directory exists
    if (!existsSync(skillsDir)) {
      try {
        mkdirSync(skillsDir, { recursive: true });
      } catch (e) {
        return yield* Effect.fail(
          new SkillStoreError("io_error", `Failed to create skills directory: ${e}`),
        );
      }
    }

    // Load existing skills
    const loadSkills = (): Effect.Effect<Map<string, Skill>, SkillStoreError> =>
      Effect.gen(function* () {
        const skills = new Map<string, Skill>();

        if (!existsSync(libraryPath)) {
          return skills;
        }

        try {
          const content = readFileSync(libraryPath, "utf-8");
          const lines = content.split("\n").filter((line) => line.trim());

          for (const line of lines) {
            try {
              const parsed = JSON.parse(line);
              const decoded = S.decodeUnknownSync(SkillSchema)(parsed);
              skills.set(decoded.id, decoded);
            } catch {
              // Skip invalid lines, log warning
              console.warn(`[SkillStore] Skipping invalid skill line: ${line.slice(0, 50)}...`);
            }
          }
        } catch (e) {
          return yield* Effect.fail(
            new SkillStoreError("io_error", `Failed to read skills library: ${e}`),
          );
        }

        return skills;
      });

    // Build index from skills
    const buildIndex = (skills: Map<string, Skill>): SkillIndex => {
      const index = createEmptyIndex();
      let lineNum = 0;

      for (const [id, skill] of skills) {
        index.byId[id] = lineNum++;
        index.count++;

        // Index by category
        if (!index.byCategory[skill.category]) {
          index.byCategory[skill.category] = [];
        }
        index.byCategory[skill.category].push(id);

        // Index by tags
        for (const tag of skill.tags ?? []) {
          if (!index.byTag[tag]) {
            index.byTag[tag] = [];
          }
          index.byTag[tag].push(id);
        }
      }

      index.lastUpdated = new Date().toISOString();
      return index;
    };

    // Save all skills (rewrite entire file)
    const saveAll = (skills: Map<string, Skill>): Effect.Effect<void, SkillStoreError> =>
      Effect.gen(function* () {
        try {
          const lines = Array.from(skills.values()).map((s) => JSON.stringify(s));
          writeFileSync(libraryPath, lines.join("\n") + "\n");

          const index = buildIndex(skills);
          writeFileSync(indexPath, JSON.stringify(index, null, 2));
        } catch (e) {
          return yield* Effect.fail(
            new SkillStoreError("io_error", `Failed to save skills library: ${e}`),
          );
        }
      });

    // Initialize state
    const initialSkills = yield* loadSkills();
    const initialIndex = buildIndex(initialSkills);

    const stateRef = yield* Ref.make<StoreState>({
      skills: initialSkills,
      index: initialIndex,
      dirty: false,
    });

    // --- Store Methods ---

    const get = (id: string): Effect.Effect<Skill | null, SkillStoreError> =>
      Ref.get(stateRef).pipe(Effect.map((state) => state.skills.get(id) ?? null));

    const list = (filter?: SkillFilter): Effect.Effect<Skill[], SkillStoreError> =>
      Ref.get(stateRef).pipe(
        Effect.map((state) => {
          let skills = Array.from(state.skills.values());

          // Apply filters
          if (filter?.categories?.length) {
            skills = skills.filter((s) => filter.categories!.includes(s.category));
          }
          if (filter?.status?.length) {
            skills = skills.filter((s) => filter.status!.includes(s.status));
          }
          if (filter?.tags?.length) {
            skills = skills.filter((s) => s.tags?.some((t) => filter.tags!.includes(t)));
          }
          if (filter?.languages?.length) {
            skills = skills.filter((s) => s.languages?.some((l) => filter.languages!.includes(l)));
          }
          if (filter?.frameworks?.length) {
            skills = skills.filter((s) =>
              s.frameworks?.some((f) => filter.frameworks!.includes(f)),
            );
          }
          if (filter?.minSuccessRate !== undefined) {
            skills = skills.filter(
              (s) => (s.successRate ?? 0) >= filter.minSuccessRate!,
            );
          }

          // Sort by success rate (descending) then by usage count
          skills.sort((a, b) => {
            const rateA = a.successRate ?? 0;
            const rateB = b.successRate ?? 0;
            if (rateA !== rateB) return rateB - rateA;
            return (b.usageCount ?? 0) - (a.usageCount ?? 0);
          });

          // Apply limit
          if (filter?.maxResults) {
            skills = skills.slice(0, filter.maxResults);
          }

          return skills;
        }),
      );

    const add = (skill: Skill): Effect.Effect<void, SkillStoreError> =>
      Effect.gen(function* () {
        const state = yield* Ref.get(stateRef);

        if (state.skills.has(skill.id)) {
          return yield* Effect.fail(
            new SkillStoreError("duplicate", `Skill with ID ${skill.id} already exists`),
          );
        }

        // Add to memory
        state.skills.set(skill.id, skill);

        // Append to file
        try {
          appendFileSync(libraryPath, JSON.stringify(skill) + "\n");
        } catch (e) {
          // Rollback memory change
          state.skills.delete(skill.id);
          return yield* Effect.fail(
            new SkillStoreError("io_error", `Failed to append skill: ${e}`),
          );
        }

        // Update index
        const newIndex = buildIndex(state.skills);
        yield* Ref.set(stateRef, { ...state, index: newIndex });
      });

    const update = (skill: Skill): Effect.Effect<void, SkillStoreError> =>
      Effect.gen(function* () {
        const state = yield* Ref.get(stateRef);

        if (!state.skills.has(skill.id)) {
          return yield* Effect.fail(
            new SkillStoreError("not_found", `Skill with ID ${skill.id} not found`),
          );
        }

        // Update in memory
        state.skills.set(skill.id, { ...skill, updatedAt: new Date().toISOString() });

        // Rewrite entire file (JSONL doesn't support in-place updates)
        yield* saveAll(state.skills);

        // Update index
        const newIndex = buildIndex(state.skills);
        yield* Ref.set(stateRef, { ...state, index: newIndex, dirty: false });
      });

    const archive = (id: string): Effect.Effect<void, SkillStoreError> =>
      Effect.gen(function* () {
        const state = yield* Ref.get(stateRef);
        const skill = state.skills.get(id);

        if (!skill) {
          return yield* Effect.fail(
            new SkillStoreError("not_found", `Skill with ID ${id} not found`),
          );
        }

        // Update status to archived
        const archived: Skill = {
          ...skill,
          status: "archived" as SkillStatus,
          updatedAt: new Date().toISOString(),
        };

        state.skills.set(id, archived);
        yield* saveAll(state.skills);

        const newIndex = buildIndex(state.skills);
        yield* Ref.set(stateRef, { ...state, index: newIndex });
      });

    const getByCategory = (category: string): Effect.Effect<Skill[], SkillStoreError> =>
      Ref.get(stateRef).pipe(
        Effect.map((state) => {
          const ids = state.index.byCategory[category] ?? [];
          return ids.map((id) => state.skills.get(id)!).filter(Boolean);
        }),
      );

    const getByTag = (tag: string): Effect.Effect<Skill[], SkillStoreError> =>
      Ref.get(stateRef).pipe(
        Effect.map((state) => {
          const ids = state.index.byTag[tag] ?? [];
          return ids.map((id) => state.skills.get(id)!).filter(Boolean);
        }),
      );

    const countFn = (): Effect.Effect<number, never> =>
      Ref.get(stateRef).pipe(Effect.map((state) => state.skills.size));

    const reload = (): Effect.Effect<void, SkillStoreError> =>
      Effect.gen(function* () {
        const skills = yield* loadSkills();
        const index = buildIndex(skills);
        yield* Ref.set(stateRef, { skills, index, dirty: false });
      });

    const getPath = (): string => libraryPath;

    return {
      get,
      list,
      add,
      update,
      archive,
      getByCategory,
      getByTag,
      count: countFn,
      reload,
      getPath,
    };
  });

// --- Layer Factory ---

/**
 * Create SkillStore layer for a project.
 */
export const makeSkillStoreLayer = (
  projectRoot: string,
): Layer.Layer<SkillStore, SkillStoreError, never> =>
  Layer.effect(SkillStore, makeStore(projectRoot));

/**
 * Create SkillStore layer using current working directory.
 */
export const SkillStoreLive: Layer.Layer<SkillStore, SkillStoreError, never> =
  makeSkillStoreLayer(process.cwd());
