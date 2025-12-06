/**
 * Skill Schema Tests
 */

import { describe, test, expect } from "bun:test";
import {
  createSkill,
  generateSkillId,
  formatSkillForPrompt,
  formatSkillsForPrompt,
  type Skill,
} from "./schema.js";
import { primitiveSkills } from "./library/primitives.js";

describe("Skill Schema", () => {
  test("generateSkillId creates valid ID", () => {
    expect(generateSkillId("Fix Import Error", "v1")).toBe("skill-fix-import-error-v1");
    expect(generateSkillId("Read File", "v2")).toBe("skill-read-file-v2");
    expect(generateSkillId("Run TypeScript Check!", "v1")).toBe("skill-run-typescript-check-v1");
  });

  test("createSkill creates skill with defaults", () => {
    const skill = createSkill({
      name: "Test Skill",
      description: "A test skill",
      code: "console.log('test')",
      category: "testing",
    });

    expect(skill.id).toBe("skill-test-skill-v1");
    expect(skill.name).toBe("Test Skill");
    expect(skill.description).toBe("A test skill");
    expect(skill.category).toBe("testing");
    expect(skill.status).toBe("active");
    expect(skill.version).toBe("v1");
    expect(skill.source).toBe("manual");
    expect(skill.parameters).toEqual([]);
    expect(skill.verification.type).toBe("none");
    expect(skill.createdAt).toBeDefined();
    expect(skill.updatedAt).toBeDefined();
  });

  test("createSkill respects provided values", () => {
    const skill = createSkill({
      id: "custom-id",
      name: "Custom Skill",
      description: "Custom description",
      code: "custom code",
      category: "debugging",
      version: "v2",
      status: "draft",
      source: "learned",
      tags: ["custom", "test"],
    });

    expect(skill.id).toBe("custom-id");
    expect(skill.version).toBe("v2");
    expect(skill.status).toBe("draft");
    expect(skill.source).toBe("learned");
    expect(skill.tags).toEqual(["custom", "test"]);
  });
});

describe("Skill Formatting", () => {
  const testSkill: Skill = createSkill({
    name: "Read File",
    description: "Read contents of a file",
    code: "const content = await Bun.file(path).text();",
    category: "file_operations",
    parameters: [
      { name: "path", type: "path", description: "File path to read", required: true },
    ],
    successRate: 0.95,
  });

  test("formatSkillForPrompt includes all relevant info", () => {
    const formatted = formatSkillForPrompt(testSkill);

    expect(formatted).toContain("Read File");
    expect(formatted).toContain("skill-read-file-v1");
    expect(formatted).toContain("file_operations");
    expect(formatted).toContain("Read contents of a file");
    expect(formatted).toContain("path: path (required)");
    expect(formatted).toContain("Success Rate: 95%");
    expect(formatted).toContain("const content = await Bun.file(path).text();");
  });

  test("formatSkillsForPrompt handles empty array", () => {
    const formatted = formatSkillsForPrompt([]);
    expect(formatted).toBe("No relevant skills found.");
  });

  test("formatSkillsForPrompt formats multiple skills", () => {
    const skills = [testSkill, createSkill({
      name: "Write File",
      description: "Write to a file",
      code: "await Bun.write(path, content);",
      category: "file_operations",
    })];

    const formatted = formatSkillsForPrompt(skills);

    expect(formatted).toContain("Read File");
    expect(formatted).toContain("Write File");
    expect(formatted).toContain("---"); // Separator
  });
});

describe("Primitive Skills Library", () => {
  test("primitiveSkills has expected count", () => {
    expect(primitiveSkills.length).toBeGreaterThanOrEqual(25);
  });

  test("all primitive skills have required fields", () => {
    for (const skill of primitiveSkills) {
      expect(skill.id).toBeDefined();
      expect(skill.name).toBeDefined();
      expect(skill.description).toBeDefined();
      expect(skill.code).toBeDefined();
      expect(skill.category).toBeDefined();
      expect(skill.status).toBe("active");
      expect(skill.source).toBe("bootstrap");
      expect(skill.createdAt).toBeDefined();
      expect(skill.updatedAt).toBeDefined();
    }
  });

  test("primitive skills cover expected categories", () => {
    const categories = new Set(primitiveSkills.map(s => s.category));

    expect(categories.has("file_operations")).toBe(true);
    expect(categories.has("testing")).toBe(true);
    expect(categories.has("git")).toBe(true);
    expect(categories.has("debugging")).toBe(true);
    expect(categories.has("shell")).toBe(true);
    expect(categories.has("search")).toBe(true);
  });

  test("file operation skills are present", () => {
    const fileOps = primitiveSkills.filter(s => s.category === "file_operations");
    const names = fileOps.map(s => s.name);

    expect(names).toContain("Read File");
    expect(names).toContain("Write File");
    expect(names).toContain("Edit File");
    expect(names).toContain("Glob Files");
  });

  test("testing skills are present", () => {
    const testing = primitiveSkills.filter(s => s.category === "testing");
    const names = testing.map(s => s.name);

    expect(names).toContain("Run Tests");
    expect(names).toContain("Run Typecheck");
    expect(names).toContain("Run Lint");
  });

  test("git skills are present", () => {
    const git = primitiveSkills.filter(s => s.category === "git");
    const names = git.map(s => s.name);

    expect(names).toContain("Git Status");
    expect(names).toContain("Git Diff");
    expect(names).toContain("Git Add");
    expect(names).toContain("Git Commit");
  });
});
