/**
 * Reflexion Schema Tests
 */

import { describe, test, expect } from "bun:test";
import {
  classifyError,
  buildReflectionPrompt,
  formatReflectionsForPrompt,
  createFailureContext,
  createReflection,
  generateFailureId,
  generateReflectionId,
} from "./schema.js";

describe("Error Classification", () => {
  test("classifyError identifies import errors", () => {
    expect(classifyError("Cannot find module 'effect'")).toBe("import_error");
    expect(classifyError("TS2307: Cannot find module")).toBe("import_error");
    expect(classifyError("import { Foo } from 'bar' failed")).toBe("import_error");
  });

  test("classifyError identifies type errors", () => {
    expect(classifyError("Type 'string' is not assignable to type 'number'")).toBe("type_error");
    expect(classifyError("TS2345: Argument of type")).toBe("type_error");
  });

  test("classifyError identifies syntax errors", () => {
    expect(classifyError("SyntaxError: Unexpected token")).toBe("syntax_error");
    expect(classifyError("Parsing error: unexpected")).toBe("syntax_error");
  });

  test("classifyError identifies test failures", () => {
    expect(classifyError("Test failed: expected true")).toBe("test_failure");
    expect(classifyError("expect(x).toBe(y) failed")).toBe("test_failure");
  });

  test("classifyError identifies timeout", () => {
    expect(classifyError("Task timed out after 60s")).toBe("timeout");
    expect(classifyError("Timeout exceeded")).toBe("timeout");
  });

  test("classifyError identifies runtime errors", () => {
    expect(classifyError("RuntimeError: undefined is not a function")).toBe("runtime_error");
    expect(classifyError("Error: something went wrong")).toBe("runtime_error");
  });

  test("classifyError returns unknown for unclassified", () => {
    expect(classifyError("Something weird happened")).toBe("unknown");
  });
});

describe("ID Generation", () => {
  test("generateFailureId creates unique IDs", () => {
    const id1 = generateFailureId();
    const id2 = generateFailureId();

    expect(id1).toMatch(/^fail-[a-z0-9]+-[a-z0-9]+$/);
    expect(id2).toMatch(/^fail-[a-z0-9]+-[a-z0-9]+$/);
    expect(id1).not.toBe(id2);
  });

  test("generateReflectionId creates unique IDs", () => {
    const id1 = generateReflectionId();
    const id2 = generateReflectionId();

    expect(id1).toMatch(/^refl-[a-z0-9]+-[a-z0-9]+$/);
    expect(id2).toMatch(/^refl-[a-z0-9]+-[a-z0-9]+$/);
    expect(id1).not.toBe(id2);
  });
});

describe("Failure Context", () => {
  test("createFailureContext creates with defaults", () => {
    const failure = createFailureContext(
      "Fix the import error",
      "Cannot find module 'effect'",
    );

    expect(failure.id).toMatch(/^fail-/);
    expect(failure.taskDescription).toBe("Fix the import error");
    expect(failure.attemptDescription).toBe("Fix the import error");
    expect(failure.errorMessage).toBe("Cannot find module 'effect'");
    expect(failure.errorType).toBe("import_error");
    expect(failure.filesInvolved).toEqual([]);
    expect(failure.attemptNumber).toBe(1);
    expect(failure.timestamp).toBeDefined();
  });

  test("createFailureContext respects options", () => {
    const failure = createFailureContext(
      "Fix the bug",
      "Type error",
      {
        attemptDescription: "Tried to fix types",
        filesInvolved: ["src/foo.ts"],
        codeWritten: "const x = 1;",
        skillsUsed: ["skill-1"],
        attemptNumber: 3,
        durationMs: 5000,
        projectId: "proj-123",
      },
    );

    expect(failure.attemptDescription).toBe("Tried to fix types");
    expect(failure.filesInvolved).toEqual(["src/foo.ts"]);
    expect(failure.codeWritten).toBe("const x = 1;");
    expect(failure.skillsUsed).toEqual(["skill-1"]);
    expect(failure.attemptNumber).toBe(3);
    expect(failure.durationMs).toBe(5000);
    expect(failure.projectId).toBe("proj-123");
  });
});

describe("Reflection", () => {
  test("createReflection creates with required fields", () => {
    const reflection = createReflection("fail-123", {
      whatWentWrong: "Import path was wrong",
      whyItWentWrong: "Relative path doesn't exist",
      whatToTryNext: "Use absolute path",
    });

    expect(reflection.id).toMatch(/^refl-/);
    expect(reflection.failureId).toBe("fail-123");
    expect(reflection.whatWentWrong).toBe("Import path was wrong");
    expect(reflection.whyItWentWrong).toBe("Relative path doesn't exist");
    expect(reflection.whatToTryNext).toBe("Use absolute path");
    expect(reflection.lessonsLearned).toEqual([]);
    expect(reflection.confidence).toBe(0.7);
    expect(reflection.timestamp).toBeDefined();
  });

  test("createReflection includes optional fields", () => {
    const reflection = createReflection("fail-456", {
      whatWentWrong: "Wrong type",
      whyItWentWrong: "Missing cast",
      whatToTryNext: "Add type assertion",
      suggestedFix: "value as string",
      lessonsLearned: ["Always check types", "Use strict mode"],
      confidence: 0.9,
    });

    expect(reflection.suggestedFix).toBe("value as string");
    expect(reflection.lessonsLearned).toEqual(["Always check types", "Use strict mode"]);
    expect(reflection.confidence).toBe(0.9);
  });
});

describe("Reflection Prompt", () => {
  test("buildReflectionPrompt includes all context", () => {
    const failure = createFailureContext(
      "Add user authentication",
      "Type 'undefined' is not assignable to type 'User'",
      {
        filesInvolved: ["src/auth.ts"],
        codeWritten: "const user = getUser();",
        skillsUsed: ["skill-get-user-v1"],
      },
    );

    const prompt = buildReflectionPrompt(failure);

    expect(prompt).toContain("Add user authentication");
    expect(prompt).toContain("Type 'undefined' is not assignable");
    expect(prompt).toContain("src/auth.ts");
    expect(prompt).toContain("const user = getUser()");
    expect(prompt).toContain("skill-get-user-v1");
    expect(prompt).toContain("What went wrong");
    expect(prompt).toContain("Why it went wrong");
    expect(prompt).toContain("What to try next");
  });
});

describe("Reflection Formatting", () => {
  test("formatReflectionsForPrompt handles empty array", () => {
    const formatted = formatReflectionsForPrompt([]);
    expect(formatted).toBe("");
  });

  test("formatReflectionsForPrompt formats single reflection", () => {
    const reflection = createReflection("fail-1", {
      whatWentWrong: "Import failed",
      whyItWentWrong: "Wrong path",
      whatToTryNext: "Fix the path",
      suggestedFix: "import from './correct'",
      lessonsLearned: ["Check paths"],
    });

    const formatted = formatReflectionsForPrompt([reflection]);

    expect(formatted).toContain("Previous Attempt Reflections");
    expect(formatted).toContain("What went wrong");
    expect(formatted).toContain("Import failed");
    expect(formatted).toContain("Wrong path");
    expect(formatted).toContain("Fix the path");
    expect(formatted).toContain("import from './correct'");
    expect(formatted).toContain("Check paths");
  });

  test("formatReflectionsForPrompt formats multiple reflections", () => {
    const reflections = [
      createReflection("fail-1", {
        whatWentWrong: "First issue",
        whyItWentWrong: "First cause",
        whatToTryNext: "First fix",
      }),
      createReflection("fail-2", {
        whatWentWrong: "Second issue",
        whyItWentWrong: "Second cause",
        whatToTryNext: "Second fix",
      }),
    ];

    const formatted = formatReflectionsForPrompt(reflections);

    expect(formatted).toContain("Reflection 1");
    expect(formatted).toContain("Reflection 2");
    expect(formatted).toContain("First issue");
    expect(formatted).toContain("Second issue");
  });
});
