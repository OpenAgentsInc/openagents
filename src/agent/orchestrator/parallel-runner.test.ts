import { describe, expect, test } from "bun:test";
import { calculateSafeMaxAgents } from "./parallel-runner.js";

describe("calculateSafeMaxAgents", () => {
  test("caps agents based on host memory and reserve", () => {
    const totalMem = 16 * 1024 * 1024 * 1024; // 16GB
    const perAgentMb = 4096;
    const reserveMb = 6144;

    // (16GB - 6.144GB) / 4GB = 2.5 -> 2 agents minimum 1
    expect(calculateSafeMaxAgents(totalMem, perAgentMb, reserveMb)).toBe(2);
  });

  test("never returns less than 1 even if reserve exceeds total", () => {
    const totalMem = 4 * 1024 * 1024 * 1024; // 4GB
    const perAgentMb = 2048;
    const reserveMb = 8192; // reserve larger than total

    expect(calculateSafeMaxAgents(totalMem, perAgentMb, reserveMb)).toBe(1);
  });
});
