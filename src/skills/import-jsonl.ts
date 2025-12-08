/**
 * One-time migration script to import existing JSONL data into SQLite
 *
 * Usage:
 *   bun src/skills/import-jsonl.ts <project-root>
 */

import { Database } from "bun:sqlite";
import { existsSync, readFileSync, mkdirSync } from "fs";
import { join, dirname } from "path";
import * as S from "effect/Schema";
import { Skill as SkillSchema, type Skill } from "./schema.js";
import { SKILLS_SCHEMA_SQL } from "./migrations.js";

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

const importJsonlToSqlite = (jsonlPath: string, dbPath: string): void => {
  if (!existsSync(jsonlPath)) {
    console.log(`[Import] JSONL file not found: ${jsonlPath}`);
    return;
  }

  console.log(`[Import] Reading JSONL from ${jsonlPath}...`);

  const content = readFileSync(jsonlPath, "utf-8");
  const lines = content.split("\n").filter((l) => l.trim());

  if (lines.length === 0) {
    console.log("[Import] No skills found in JSONL file");
    return;
  }

  // Ensure database directory exists
  const dbDir = dirname(dbPath);
  if (!existsSync(dbDir)) {
    mkdirSync(dbDir, { recursive: true });
  }

  // Open database
  const db = new Database(dbPath);

  // Run migrations
  console.log("[Import] Running migrations...");
  db.exec(SKILLS_SCHEMA_SQL);

  // Prepare insert statement
  const insertStmt = db.prepare(`
    INSERT OR REPLACE INTO skills (
      id, name, version, description, category, status, code,
      parameters, prerequisites, postconditions, examples, tags,
      languages, frameworks, learned_from, verification,
      embedding, success_rate, usage_count, last_used, source,
      created_at, updated_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  `);

  let imported = 0;
  let skipped = 0;

  // Import in transaction
  db.transaction(() => {
    for (const line of lines) {
      try {
        const parsed = JSON.parse(line);
        const skill = S.decodeUnknownSync(SkillSchema)(parsed);
        const row = skillToRow(skill);
        insertStmt.run(...row);
        imported++;
      } catch (e) {
        console.warn(`[Import] Skipping invalid skill line: ${line.slice(0, 50)}...`);
        skipped++;
      }
    }
  })();

  console.log(`[Import] Imported ${imported} skills, skipped ${skipped} invalid lines`);
  console.log(`[Import] Database saved to ${dbPath}`);
};

// Main execution
const projectRoot = process.argv[2] || process.cwd();
const jsonlPath = join(projectRoot, ".openagents", "skills", "library.jsonl");
const dbPath = join(projectRoot, ".openagents", "skills.db");

console.log(`[Import] Migrating skills from JSONL to SQLite...`);
console.log(`[Import] Project root: ${projectRoot}`);
console.log(`[Import] JSONL path: ${jsonlPath}`);
console.log(`[Import] Database path: ${dbPath}`);

importJsonlToSqlite(jsonlPath, dbPath);

console.log("[Import] Migration complete!");
