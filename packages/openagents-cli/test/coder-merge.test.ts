import { describe, expect, it } from "vitest";

import { merge } from "../src/coder-merge.js";

const after = (ms: number, ...items: string[]): AsyncIterable<string> => ({
  async *[Symbol.asyncIterator]() {
    for (const item of items) {
      await new Promise((resolve) => setTimeout(resolve, ms));
      yield item;
    }
  },
});

describe("running streams together", () => {
  it("yields in arrival order, not in the order the streams were given", async () => {
    const seen: string[] = [];
    for await (const item of merge([after(40, "slow"), after(5, "fast")])) seen.push(item);

    // The whole point: a fan-out to two models should cost the slower of the
    // two rather than the sum.
    expect(seen).toEqual(["fast", "slow"]);
  });

  it("takes the time of the slowest, not the total", async () => {
    const started = Date.now();
    for await (const _ of merge([after(60, "a"), after(60, "b"), after(60, "c")])) void _;

    expect(Date.now() - started).toBeLessThan(150);
  });

  it("keeps each stream's own order", async () => {
    const seen: string[] = [];
    for await (const item of merge([after(10, "1", "2", "3")])) seen.push(item);

    // A tool call and its result are a sequence, whatever else is running.
    expect(seen).toEqual(["1", "2", "3"]);
  });

  it("lets the others finish before it raises a failure", async () => {
    const angry: AsyncIterable<string> = {
      // eslint-disable-next-line require-yield -- it only throws
      async *[Symbol.asyncIterator]() {
        throw new Error("tool exploded");
      },
    };
    const seen: string[] = [];

    await expect(
      (async () => {
        for await (const item of merge([angry, after(20, "survivor")])) seen.push(item);
      })(),
    ).rejects.toThrow("tool exploded");

    // A tool that threw must not strand the others mid-flight.
    expect(seen).toEqual(["survivor"]);
  });

  it("passes a single stream straight through", async () => {
    const seen: string[] = [];
    for await (const item of merge([after(1, "only")])) seen.push(item);
    expect(seen).toEqual(["only"]);
  });
});
