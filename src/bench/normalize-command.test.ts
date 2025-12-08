import { describe, expect, it } from "bun:test";
import { normalizeCommand } from "./model-adapter.js";

describe("normalizeCommand", () => {
  it("replaces /app/ with ./", () => {
    expect(normalizeCommand("touch /app/file.txt")).toBe("touch ./file.txt");
  });

  it("strips cd /app && prefix", () => {
    expect(normalizeCommand("cd /app && ./configure")).toBe("./configure");
  });

  it("strips cd /app; prefix", () => {
    expect(normalizeCommand("cd /app; make")).toBe("make");
  });

  it("handles multiple /app/ occurrences", () => {
    expect(normalizeCommand("cp /app/a.txt /app/b.txt")).toBe("cp ./a.txt ./b.txt");
  });

  it("leaves relative paths unchanged", () => {
    expect(normalizeCommand("touch ./file.txt")).toBe("touch ./file.txt");
  });

  it("handles complex command with cd /app; and multiple /app/ paths", () => {
    const cmd = "cd /app; cp /app/source.c /app/build/ && gcc /app/build/source.c -o /app/output";
    const result = normalizeCommand(cmd);
    expect(result).toBe("cp ./source.c ./build/ && gcc ./build/source.c -o ./output");
    expect(result).not.toContain("/app");
  });

  it("handles case-insensitive cd /app", () => {
    expect(normalizeCommand("CD /APP && ls")).toBe("ls");
    expect(normalizeCommand("Cd /App; pwd")).toBe("pwd");
  });

  it("handles cd /app with extra whitespace", () => {
    expect(normalizeCommand("cd   /app   &&   ls")).toBe("ls");
    expect(normalizeCommand("cd /app   ;   make")).toBe("make");
  });
});
