import * as BunContext from "@effect/platform-bun/BunContext";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect, Option } from "effect";
import {
  detectPlatform,
  getCacheDir,
  getCachedToolPath,
  findInPath,
  findInCache,
  getDownloadUrl,
  getToolPath,
} from "./tools-manager.js";

const runWithBun = <A, E>(
  program: Effect.Effect<
    A,
    E,
    CommandExecutor.CommandExecutor | FileSystem.FileSystem | Path.Path
  >,
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("tools-manager", () => {
  describe("detectPlatform", () => {
    it("returns valid platform info", () => {
      const info = detectPlatform();

      expect(["darwin", "linux", "windows"]).toContain(info.os);
      expect(["x64", "arm64"]).toContain(info.arch);
    });
  });

  describe("getCacheDir", () => {
    it("returns a path under home directory", () => {
      const cacheDir = getCacheDir();

      expect(cacheDir).toContain(".openagents");
      expect(cacheDir).toContain("bin");
    });
  });

  describe("getCachedToolPath", () => {
    it("returns correct path for rg", () => {
      const path = getCachedToolPath("rg");
      const info = detectPlatform();

      expect(path).toContain(".openagents");
      expect(path).toContain("bin");

      if (info.os === "windows") {
        expect(path).toEndWith("rg.exe");
      } else {
        expect(path).toEndWith("rg");
      }
    });

    it("returns correct path for fd", () => {
      const path = getCachedToolPath("fd");
      const info = detectPlatform();

      expect(path).toContain(".openagents");
      expect(path).toContain("bin");

      if (info.os === "windows") {
        expect(path).toEndWith("fd.exe");
      } else {
        expect(path).toEndWith("fd");
      }
    });
  });

  describe("getDownloadUrl", () => {
    it("returns valid GitHub URL for rg", () => {
      const url = getDownloadUrl("rg");

      expect(url).toContain("github.com");
      expect(url).toContain("BurntSushi/ripgrep");
      expect(url).toContain("releases/download");
      expect(url).toContain("14.1.1");
    });

    it("returns valid GitHub URL for fd", () => {
      const url = getDownloadUrl("fd");

      expect(url).toContain("github.com");
      expect(url).toContain("sharkdp/fd");
      expect(url).toContain("releases/download");
      expect(url).toContain("10.2.0");
    });

    it("uses correct platform suffix", () => {
      const url = getDownloadUrl("rg");
      const info = detectPlatform();

      if (info.os === "darwin") {
        expect(url).toContain("apple-darwin");
      } else if (info.os === "linux") {
        expect(url).toContain("linux-musl");
      } else {
        expect(url).toContain("windows-msvc");
      }
    });

    it("uses correct architecture", () => {
      const url = getDownloadUrl("rg");
      const info = detectPlatform();

      if (info.arch === "arm64") {
        expect(url).toContain("aarch64");
      } else {
        expect(url).toContain("x86_64");
      }
    });
  });

  describe("findInPath", () => {
    it("finds rg if available in PATH", async () => {
      const result = await runWithBun(findInPath("rg"));

      // rg may or may not be in PATH, but the function should not throw
      expect(Option.isOption(result)).toBe(true);
    });

    it("finds fd if available in PATH", async () => {
      const result = await runWithBun(findInPath("fd"));

      // fd may or may not be in PATH, but the function should not throw
      expect(Option.isOption(result)).toBe(true);
    });
  });

  describe("findInCache", () => {
    it("returns None when cache does not exist", async () => {
      const result = await runWithBun(findInCache("rg"));

      // Unless we've previously cached rg, this should be None
      // The test just checks the function works without throwing
      expect(Option.isOption(result)).toBe(true);
    });
  });

  describe("getToolPath", () => {
    it("returns path or null for rg", async () => {
      const path = await runWithBun(getToolPath("rg"));

      // Path is either a string or null
      expect(path === null || typeof path === "string").toBe(true);

      if (path !== null) {
        expect(path.length).toBeGreaterThan(0);
      }
    });

    it("returns path or null for fd", async () => {
      const path = await runWithBun(getToolPath("fd"));

      // Path is either a string or null
      expect(path === null || typeof path === "string").toBe(true);

      if (path !== null) {
        expect(path.length).toBeGreaterThan(0);
      }
    });
  });
});
