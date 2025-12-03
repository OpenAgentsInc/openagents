import { describe, test, expect, beforeAll, afterAll } from "bun:test";
import { mkdirSync, writeFileSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import {
  parseFrontmatter,
  loadCommand,
  loadCommandsFromDir,
  loadAllCommands,
  substituteArgs,
  parseSlashInput,
  expandSlashCommand,
  isSlashCommand,
  listCommands,
} from "./slash-commands.js";

describe("slash-commands", () => {
  describe("parseFrontmatter", () => {
    test("parses simple frontmatter with description", () => {
      const content = `---
description: Run tests
---
Run all the tests`;

      const result = parseFrontmatter(content);
      expect(result.frontmatter.description).toBe("Run tests");
      expect(result.body).toBe("Run all the tests");
    });

    test("handles content without frontmatter", () => {
      const content = "Just a simple command body";
      const result = parseFrontmatter(content);
      expect(result.frontmatter).toEqual({});
      expect(result.body).toBe("Just a simple command body");
    });

    test("parses frontmatter with args array", () => {
      const content = `---
description: Test a file
args:
  - name: file
    description: The file to test
    required: true
  - name: verbose
    description: Enable verbose output
    required: false
---
Test the file: $1`;

      const result = parseFrontmatter(content);
      expect(result.frontmatter.description).toBe("Test a file");
      expect(result.frontmatter.args).toEqual([
        { name: "file", description: "The file to test", required: true },
        { name: "verbose", description: "Enable verbose output", required: false },
      ]);
      expect(result.body).toBe("Test the file: $1");
    });

    test("handles leading whitespace before frontmatter", () => {
      const content = `
---
description: Trimmed
---
Body here`;

      const result = parseFrontmatter(content);
      expect(result.frontmatter.description).toBe("Trimmed");
      expect(result.body).toBe("Body here");
    });

    test("handles unclosed frontmatter delimiter", () => {
      const content = `---
description: Never closed
Just body content`;

      const result = parseFrontmatter(content);
      expect(result.frontmatter).toEqual({});
    });
  });

  describe("substituteArgs", () => {
    test("substitutes positional arguments", () => {
      const body = "Test $1 and $2";
      const result = substituteArgs(body, ["foo.ts", "bar.ts"]);
      expect(result).toBe("Test foo.ts and bar.ts");
    });

    test("substitutes $@ with all arguments", () => {
      const body = "Run: $@";
      const result = substituteArgs(body, ["--verbose", "--watch", "src/"]);
      expect(result).toBe("Run: --verbose --watch src/");
    });

    test("substitutes $ARGS same as $@", () => {
      const body = "Args: $ARGS";
      const result = substituteArgs(body, ["a", "b", "c"]);
      expect(result).toBe("Args: a b c");
    });

    test("handles missing positional arguments with empty string", () => {
      const body = "First: $1, Second: $2, Third: $3";
      const result = substituteArgs(body, ["only-one"]);
      expect(result).toBe("First: only-one, Second: , Third: ");
    });

    test("handles multiple occurrences of same argument", () => {
      const body = "$1 $1 $1";
      const result = substituteArgs(body, ["repeat"]);
      expect(result).toBe("repeat repeat repeat");
    });

    test("handles empty args", () => {
      const body = "No args: $@ end";
      const result = substituteArgs(body, []);
      expect(result).toBe("No args:  end");
    });
  });

  describe("parseSlashInput", () => {
    test("parses simple command", () => {
      const result = parseSlashInput("/test");
      expect(result).toEqual({ name: "test", args: [] });
    });

    test("parses command with arguments", () => {
      const result = parseSlashInput("/test foo.ts bar.ts");
      expect(result).toEqual({ name: "test", args: ["foo.ts", "bar.ts"] });
    });

    test("handles extra whitespace", () => {
      const result = parseSlashInput("  /test   arg1   arg2  ");
      expect(result).toEqual({ name: "test", args: ["arg1", "arg2"] });
    });

    test("returns null for non-slash input", () => {
      const result = parseSlashInput("not a slash command");
      expect(result).toBeNull();
    });

    test("returns null for empty slash", () => {
      const result = parseSlashInput("/");
      expect(result).toBeNull();
    });
  });

  describe("isSlashCommand", () => {
    test("returns true for slash command", () => {
      expect(isSlashCommand("/test")).toBe(true);
      expect(isSlashCommand("  /test")).toBe(true);
    });

    test("returns false for non-slash command", () => {
      expect(isSlashCommand("test")).toBe(false);
      expect(isSlashCommand("")).toBe(false);
    });
  });

  describe("file operations", () => {
    const testDir = join(tmpdir(), `slash-commands-test-${Date.now()}`);
    const userDir = join(testDir, "user", ".openagents", "commands");
    const projectDir = join(testDir, "project", ".openagents", "commands");

    beforeAll(() => {
      // Create test directories
      mkdirSync(userDir, { recursive: true });
      mkdirSync(projectDir, { recursive: true });

      // Create user command
      writeFileSync(
        join(userDir, "greet.md"),
        `---
description: Greet someone
args:
  - name: name
    required: true
---
Hello, $1!`,
      );

      // Create project command (same name, should override)
      writeFileSync(
        join(projectDir, "greet.md"),
        `---
description: Project greeting
---
Welcome to the project, $1!`,
      );

      // Create project-only command
      writeFileSync(
        join(projectDir, "build.md"),
        `---
description: Build the project
---
Running build with: $@`,
      );
    });

    afterAll(() => {
      rmSync(testDir, { recursive: true, force: true });
    });

    test("loadCommand loads a command file", () => {
      const cmd = loadCommand(join(userDir, "greet.md"), "user");
      expect(cmd).not.toBeNull();
      expect(cmd!.name).toBe("greet");
      expect(cmd!.description).toBe("Greet someone");
      expect(cmd!.source).toBe("user");
      expect(cmd!.body).toBe("Hello, $1!");
    });

    test("loadCommand returns null for non-existent file", () => {
      const cmd = loadCommand(join(userDir, "nonexistent.md"), "user");
      expect(cmd).toBeNull();
    });

    test("loadCommandsFromDir loads all commands", () => {
      const commands = loadCommandsFromDir(projectDir, "project");
      expect(commands.length).toBe(2);
      expect(commands.map((c) => c.name).sort()).toEqual(["build", "greet"]);
    });

    test("loadCommandsFromDir returns empty for non-existent dir", () => {
      const commands = loadCommandsFromDir("/nonexistent/path", "user");
      expect(commands).toEqual([]);
    });

    test("loadAllCommands merges user and project commands", () => {
      // We need to mock the home dir for this test
      // Instead, test with explicit project dir
      const commands = loadAllCommands(join(testDir, "project"));

      // Should have both greet and build
      expect(commands.has("greet")).toBe(true);
      expect(commands.has("build")).toBe(true);

      // greet should be project version (overrides user)
      const greet = commands.get("greet");
      expect(greet!.source).toBe("project");
      expect(greet!.description).toBe("Project greeting");
    });

    test("expandSlashCommand expands with arguments", () => {
      const commands = loadAllCommands(join(testDir, "project"));
      const result = expandSlashCommand("/greet Alice", commands);

      expect(result).not.toBeNull();
      expect(result!.command.name).toBe("greet");
      expect(result!.arguments).toEqual(["Alice"]);
      expect(result!.prompt).toBe("Welcome to the project, Alice!");
    });

    test("expandSlashCommand returns null for unknown command", () => {
      const commands = loadAllCommands(join(testDir, "project"));
      const result = expandSlashCommand("/unknown", commands);
      expect(result).toBeNull();
    });

    test("listCommands returns command info", () => {
      const commands = loadAllCommands(join(testDir, "project"));
      const list = listCommands(commands);

      expect(list.length).toBe(2);
      const greet = list.find((c) => c.name === "greet");
      expect(greet).toEqual({
        name: "greet",
        description: "Project greeting",
        source: "project",
      });
    });
  });
});
