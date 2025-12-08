/**
 * Skill Library Store
 *
 * SQLite-based storage for skills.
 * Location: .openagents/skills.db
 */

import { Database } from "bun:sqlite";
import { Effect, Context, Layer } from "effect";
import * as S from "effect/Schema";
import { existsSync, mkdirSync } from "fs";
import { join, dirname } from "path";
import {
  type Skill,
  type SkillFilter,
  Skill as SkillSchema,
} from "./schema.js";
import { migrateSkillsDatabase } from "./migrations.js";

// --- Error Types ---

export class SkillStoreError extends Error {
  readonly _tag = "SkillStoreError";
  constructor(
    readonly reason:
      | "connection"
      | "query"
      | "not_found"
      | "duplicate"
      | "migration"
      | "parse_error",
    message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "SkillStoreError";
  }
}

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
  readonly getByCategory: (
    category: string,
  ) => Effect.Effect<Skill[], SkillStoreError>;

  /** Get skills by tag */
  readonly getByTag: (tag: string) => Effect.Effect<Skill[], SkillStoreError>;

  /** Get skill count */
  readonly count: () => Effect.Effect<number, never>;

  /** Reload from disk (no-op for SQLite, kept for interface compatibility) */
  readonly reload: () => Effect.Effect<void, SkillStoreError>;

  /** Get the store path */
  readonly getPath: () => string;
}

// --- Store Tag ---

export class SkillStore extends Context.Tag("SkillStore")<
  SkillStore,
  ISkillStore
>() {}

// --- Helper Functions ---

/**
 * Serialize embedding array to BLOB
 */
const serializeEmbedding = (
  embedding: readonly number[] | number[] | undefined,
): Buffer | null => {
  if (!embedding || embedding.length === 0) {
    return null;
  }
  const buffer = Buffer.alloc(embedding.length * 4);
  // Convert readonly array to mutable for forEach
  const mutableEmbedding = [...embedding];
  mutableEmbedding.forEach((val, i) => buffer.writeFloatLE(val, i * 4));
  return buffer;
};

/**
 * Deserialize BLOB to embedding array
 */
const deserializeEmbedding = (blob: Buffer | null): number[] | undefined => {
  if (!blob || blob.length === 0) {
    return undefined;
  }
  const result: number[] = [];
  for (let i = 0; i < blob.length; i += 4) {
    result.push(blob.readFloatLE(i));
  }
  return result;
};

/**
 * Convert database row to Skill object
 */
const rowToSkill = (row: any): Skill => {
  const skill: Skill = {
    id: row.id,
    name: row.name,
    version: row.version,
    description: row.description,
    category: row.category,
    status: row.status,
    code: row.code,
    parameters: row.parameters ? JSON.parse(row.parameters) : [],
    prerequisites: row.prerequisites
      ? JSON.parse(row.prerequisites)
      : undefined,
    postconditions: row.postconditions
      ? JSON.parse(row.postconditions)
      : undefined,
    examples: row.examples ? JSON.parse(row.examples) : undefined,
    tags: row.tags ? JSON.parse(row.tags) : undefined,
    languages: row.languages ? JSON.parse(row.languages) : undefined,
    frameworks: row.frameworks ? JSON.parse(row.frameworks) : undefined,
    learnedFrom: row.learned_from ? JSON.parse(row.learned_from) : undefined,
    verification: row.verification
      ? JSON.parse(row.verification)
      : { type: "none" },
    embedding: deserializeEmbedding(row.embedding),
    successRate: row.success_rate ?? undefined,
    usageCount: row.usage_count ?? undefined,
    lastUsed: row.last_used ?? undefined,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    source: row.source ?? undefined,
  };

  // Validate with schema
  return S.decodeUnknownSync(SkillSchema)(skill);
};

/**
 * Convert Skill object to database row values
 */
const skillToRow = (skill: Skill): any[] => {
  return [
    skill.id,
    skill.name,
    skill.version,
    skill.description,
    skill.category,
    skill.status,
    skill.code,
    JSON.stringify(skill.parameters ?? []),
    JSON.stringify(skill.prerequisites ?? []),
    JSON.stringify(skill.postconditions ?? []),
    JSON.stringify(skill.examples ?? []),
    JSON.stringify(skill.tags ?? []),
    JSON.stringify(skill.languages ?? []),
    JSON.stringify(skill.frameworks ?? []),
    JSON.stringify(skill.learnedFrom ?? []),
    JSON.stringify(skill.verification),
    serializeEmbedding(skill.embedding),
    skill.successRate ?? null,
    skill.usageCount ?? null,
    skill.lastUsed ?? null,
    skill.source ?? null,
    skill.createdAt,
    skill.updatedAt,
  ];
};

/**
 * Build WHERE clause from filter
 */
const buildWhereClause = (
  filter?: SkillFilter,
): { sql: string; params: any[] } => {
  const conditions: string[] = [];
  const params: any[] = [];

  if (filter?.categories && filter.categories.length > 0) {
    const placeholders = filter.categories.map(() => "?").join(",");
    conditions.push(`category IN (${placeholders})`);
    params.push(...filter.categories);
  }

  if (filter?.status && filter.status.length > 0) {
    const placeholders = filter.status.map(() => "?").join(",");
    conditions.push(`status IN (${placeholders})`);
    params.push(...filter.status);
  }

  if (filter?.tags && filter.tags.length > 0) {
    // Check if JSON array contains any of the tags
    const tagConditions = filter.tags.map(
      () => `json_extract(tags, '$') LIKE ?`,
    );
    conditions.push(`(${tagConditions.join(" OR ")})`);
    params.push(...filter.tags.map((t) => `%"${t}"%`));
  }

  if (filter?.languages && filter.languages.length > 0) {
    const langConditions = filter.languages.map(
      () => `json_extract(languages, '$') LIKE ?`,
    );
    conditions.push(`(${langConditions.join(" OR ")})`);
    params.push(...filter.languages.map((l) => `%"${l}"%`));
  }

  if (filter?.frameworks && filter.frameworks.length > 0) {
    const frameworkConditions = filter.frameworks.map(
      () => `json_extract(frameworks, '$') LIKE ?`,
    );
    conditions.push(`(${frameworkConditions.join(" OR ")})`);
    params.push(...filter.frameworks.map((f) => `%"${f}"%`));
  }

  if (filter?.minSuccessRate !== undefined) {
    conditions.push("success_rate >= ?");
    params.push(filter.minSuccessRate);
  }

  const sql = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";
  return { sql, params };
};

// --- Store Implementation ---

const makeStore = (
  projectRoot: string,
): Effect.Effect<ISkillStore, SkillStoreError> =>
  Effect.gen(function* () {
    const dbPath = join(projectRoot, ".openagents", "skills.db");

    // Ensure directory exists
    const dbDir = dirname(dbPath);
    if (!existsSync(dbDir)) {
      try {
        mkdirSync(dbDir, { recursive: true });
      } catch (e) {
        return yield* Effect.fail(
          new SkillStoreError(
            "connection",
            `Failed to create database directory: ${e}`,
            e,
          ),
        );
      }
    }

    // Open/create database
    const db = yield* Effect.try({
      try: () => new Database(dbPath),
      catch: (e) =>
        new SkillStoreError("connection", `Failed to open database: ${e}`, e),
    });

    // Run migrations
    yield* migrateSkillsDatabase(db);

    // Helper: Run SQL with error handling
    const runSQL = <T>(
      fn: () => T,
    ): Effect.Effect<T, SkillStoreError> =>
      Effect.try({
        try: fn,
        catch: (e) =>
          new SkillStoreError("query", `SQL operation failed: ${e}`, e),
      });

    // Seed bootstrap skills if database is empty
    // Skip seeding in test environments (when project root contains 'test' or is in /tmp)
    const seedBootstrapSkills = (): Effect.Effect<void, SkillStoreError> =>
      Effect.gen(function* () {
        // Skip seeding in test environments
        if (
          projectRoot.includes("/test") ||
          projectRoot.includes("/tmp") ||
          process.env.NODE_ENV === "test"
        ) {
          return;
        }

        const countStmt = db.prepare("SELECT COUNT(*) as count FROM skills");
        const count = (countStmt.get() as any).count;

        if (count > 0) {
          return; // Already seeded
        }

        // Load bootstrap skills
        const bootstrapResult = yield* Effect.tryPromise({
          try: async () => {
            const { bootstrapSkills } = await import("./library/index.js");
            return bootstrapSkills;
          },
          catch: () =>
            new SkillStoreError(
              "query",
              "Failed to load bootstrap skills",
            ) as never,
        }).pipe(Effect.orElseSucceed(() => [] as Skill[]));

        if (bootstrapResult.length === 0) {
          return;
        }

        // Insert all bootstrap skills in a transaction
        yield* runSQL(() => {
          const insertStmt = db.prepare(`
            INSERT OR IGNORE INTO skills (
              id, name, version, description, category, status, code,
              parameters, prerequisites, postconditions, examples, tags,
              languages, frameworks, learned_from, verification,
              embedding, success_rate, usage_count, last_used, source,
              created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          `);

          db.transaction(() => {
            for (const skill of bootstrapResult) {
              const row = skillToRow(skill);
              insertStmt.run(...row);
            }
          })();

          console.log(
            `[SkillStore] Seeded ${bootstrapResult.length} bootstrap skills`,
          );
        });
      });

    // Seed on startup
    yield* seedBootstrapSkills();

    // --- Store Methods ---

    const get = (
      id: string,
    ): Effect.Effect<Skill | null, SkillStoreError> =>
      runSQL(() => {
        const stmt = db.prepare("SELECT * FROM skills WHERE id = ?");
        const row = stmt.get(id) as any;

        if (!row) {
          return null;
        }

        return rowToSkill(row);
      });

    const list = (
      filter?: SkillFilter,
    ): Effect.Effect<Skill[], SkillStoreError> =>
      runSQL(() => {
        const { sql: whereClause, params } = buildWhereClause(filter);

        // Build ORDER BY clause
        let orderBy = "success_rate DESC, usage_count DESC";
        if (filter?.maxResults) {
          // SQLite doesn't support LIMIT in subqueries easily, so we'll handle it after
        }

        const stmt = db.prepare(
          `SELECT * FROM skills ${whereClause} ORDER BY ${orderBy}`,
        );
        const rows = stmt.all(...params) as any[];

        let skills = rows.map(rowToSkill);

        // Apply maxResults limit if specified
        if (filter?.maxResults) {
          skills = skills.slice(0, filter.maxResults);
        }

        return skills;
      });

    const add = (skill: Skill): Effect.Effect<void, SkillStoreError> =>
      runSQL(() => {
        // Check if skill already exists
        const checkStmt = db.prepare("SELECT id FROM skills WHERE id = ?");
        const existing = checkStmt.get(skill.id);

        if (existing) {
          throw new Error(`Skill with ID ${skill.id} already exists`);
        }

        const insertStmt = db.prepare(`
          INSERT INTO skills (
            id, name, version, description, category, status, code,
            parameters, prerequisites, postconditions, examples, tags,
            languages, frameworks, learned_from, verification,
            embedding, success_rate, usage_count, last_used, source,
            created_at, updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `);

        const row = skillToRow(skill);
        insertStmt.run(...row);
      }).pipe(
        Effect.mapError((e) => {
          if (e.message.includes("already exists")) {
            return new SkillStoreError("duplicate", e.message, e);
          }
          return e;
        }),
      );

    const update = (skill: Skill): Effect.Effect<void, SkillStoreError> =>
      runSQL(() => {
        // Check if skill exists
        const checkStmt = db.prepare("SELECT id FROM skills WHERE id = ?");
        const existing = checkStmt.get(skill.id);

        if (!existing) {
          throw new Error(`Skill with ID ${skill.id} not found`);
        }

        // Update with current timestamp
        const updatedSkill: Skill = {
          ...skill,
          updatedAt: new Date().toISOString(),
        };

        const updateStmt = db.prepare(`
          UPDATE skills SET
            name = ?, version = ?, description = ?, category = ?, status = ?, code = ?,
            parameters = ?, prerequisites = ?, postconditions = ?, examples = ?, tags = ?,
            languages = ?, frameworks = ?, learned_from = ?, verification = ?,
            embedding = ?, success_rate = ?, usage_count = ?, last_used = ?, source = ?,
            updated_at = ?
          WHERE id = ?
        `);

        const row = skillToRow(updatedSkill);
        // Remove id, createdAt from row (they're not updated)
        const updateParams = [
          row[1], // name
          row[2], // version
          row[3], // description
          row[4], // category
          row[5], // status
          row[6], // code
          row[7], // parameters
          row[8], // prerequisites
          row[9], // postconditions
          row[10], // examples
          row[11], // tags
          row[12], // languages
          row[13], // frameworks
          row[14], // learned_from
          row[15], // verification
          row[16], // embedding
          row[17], // success_rate
          row[18], // usage_count
          row[19], // last_used
          row[20], // source
          row[22], // updated_at
          row[0], // id (for WHERE clause)
        ];

        updateStmt.run(...updateParams);
      }).pipe(
        Effect.mapError((e) => {
          if (e.message.includes("not found")) {
            return new SkillStoreError("not_found", e.message, e);
          }
          return e;
        }),
      );

    const archive = (id: string): Effect.Effect<void, SkillStoreError> =>
      runSQL(() => {
        const checkStmt = db.prepare("SELECT id FROM skills WHERE id = ?");
        const existing = checkStmt.get(id);

        if (!existing) {
          throw new Error(`Skill with ID ${id} not found`);
        }

        const updateStmt = db.prepare(
          "UPDATE skills SET status = ?, updated_at = ? WHERE id = ?",
        );
        updateStmt.run("archived", new Date().toISOString(), id);
      }).pipe(
        Effect.mapError((e) => {
          if (e.message.includes("not found")) {
            return new SkillStoreError("not_found", e.message, e);
          }
          return e;
        }),
      );

    const getByCategory = (
      category: string,
    ): Effect.Effect<Skill[], SkillStoreError> =>
      runSQL(() => {
        const stmt = db.prepare(
          "SELECT * FROM skills WHERE category = ? ORDER BY success_rate DESC, usage_count DESC",
        );
        const rows = stmt.all(category) as any[];
        return rows.map(rowToSkill);
      });

    const getByTag = (tag: string): Effect.Effect<Skill[], SkillStoreError> =>
      runSQL(() => {
        const stmt = db.prepare(
          `SELECT * FROM skills WHERE json_extract(tags, '$') LIKE ? ORDER BY success_rate DESC, usage_count DESC`,
        );
        const rows = stmt.all(`%"${tag}"%`) as any[];
        return rows.map(rowToSkill);
      });

    const countFn = (): Effect.Effect<number, never> =>
      runSQL(() => {
        const stmt = db.prepare("SELECT COUNT(*) as count FROM skills");
        const result = stmt.get() as any;
        return result.count;
      }).pipe(Effect.orElse(() => Effect.succeed(0)));

    const reload = (): Effect.Effect<void, SkillStoreError> =>
      Effect.succeed(undefined); // No-op for SQLite

    const getPath = (): string => dbPath;

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
