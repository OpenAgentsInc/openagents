// Target registry tests (#6190): named targets, restrictions, env overrides,
// and the read-only enforcement policy. Pure — no browser, no network.

import { describe, expect, test } from "bun:test";

import {
  checkStepAllowed,
  isMutatingStepKind,
  isReadOnly,
  makeTarget,
} from "./target";
import {
  defaultTargetName,
  isTargetName,
  parseTargetSelection,
  resolveRegistryTarget,
  resolveSelectedTargets,
  TARGET_NAMES,
  TARGET_REGISTRY,
} from "./target-registry";

describe("registry shape", () => {
  test("has dev/staging/prod/selfhost", () => {
    expect([...TARGET_NAMES].sort()).toEqual(["dev", "prod", "selfhost", "staging"]);
    for (const name of TARGET_NAMES) {
      expect(TARGET_REGISTRY[name].name).toBe(name);
    }
  });

  test("prod is read-only by default; dev/staging/selfhost are writable", () => {
    expect(TARGET_REGISTRY.prod.restrictions).toContain("read-only");
    expect(TARGET_REGISTRY.dev.restrictions).toEqual([]);
    expect(TARGET_REGISTRY.staging.restrictions).toEqual([]);
    expect(TARGET_REGISTRY.selfhost.restrictions).toEqual([]);
  });

  test("selfhost has no baked-in base URL (operator-supplied)", () => {
    expect(TARGET_REGISTRY.selfhost.baseUrl).toBeUndefined();
  });
});

describe("resolveRegistryTarget", () => {
  test("resolves prod as read-only, dev as writable", () => {
    const prod = resolveRegistryTarget("prod", {});
    expect(prod.baseUrl).toBe("https://openagents.com");
    expect(isReadOnly(prod)).toBe(true);

    const dev = resolveRegistryTarget("dev", {});
    expect(isReadOnly(dev)).toBe(false);
  });

  test("env QA_<NAME>_URL overrides the base URL", () => {
    const dev = resolveRegistryTarget("dev", { QA_DEV_URL: "http://127.0.0.1:9999" });
    expect(dev.baseUrl).toBe("http://127.0.0.1:9999");
  });

  test("selfhost without QA_SELFHOST_URL is an HONEST error (never a guessed URL)", () => {
    expect(() => resolveRegistryTarget("selfhost", {})).toThrow(/QA_SELFHOST_URL/);
  });

  test("selfhost resolves from QA_SELFHOST_URL", () => {
    const sh = resolveRegistryTarget("selfhost", { QA_SELFHOST_URL: "http://my-host:3000" });
    expect(sh.baseUrl).toBe("http://my-host:3000");
    expect(isReadOnly(sh)).toBe(false);
  });
});

describe("defaultTargetName", () => {
  test("defaults to dev (safest writable target)", () => {
    expect(defaultTargetName({})).toBe("dev");
  });
  test("honors QA_DEFAULT_TARGET", () => {
    expect(defaultTargetName({ QA_DEFAULT_TARGET: "staging" })).toBe("staging");
  });
  test("rejects an unknown QA_DEFAULT_TARGET honestly", () => {
    expect(() => defaultTargetName({ QA_DEFAULT_TARGET: "nope" })).toThrow(/not a known target/);
  });
});

describe("resolveSelectedTargets + parseTargetSelection", () => {
  test("resolves a selection preserving order and de-duplicating", () => {
    const targets = resolveSelectedTargets(["dev", "prod", "dev"], {});
    expect(targets.map((t) => t.name)).toEqual(["dev", "prod"]);
  });

  test("unknown name in a selection is an honest error", () => {
    expect(() => resolveSelectedTargets(["dev", "qa"], {})).toThrow(/unknown target "qa"/);
  });

  test("parseTargetSelection handles comma + space separated", () => {
    expect(parseTargetSelection("dev,prod", {}).map((t) => t.name)).toEqual(["dev", "prod"]);
    expect(parseTargetSelection("dev prod", {}).map((t) => t.name)).toEqual(["dev", "prod"]);
  });

  test("empty selection -> the default target", () => {
    expect(parseTargetSelection(undefined, {}).map((t) => t.name)).toEqual(["dev"]);
    expect(parseTargetSelection("   ", { QA_DEFAULT_TARGET: "staging" }).map((t) => t.name)).toEqual([
      "staging",
    ]);
  });
});

describe("isTargetName", () => {
  test("recognizes registry names only", () => {
    expect(isTargetName("prod")).toBe(true);
    expect(isTargetName("nope")).toBe(false);
  });
});

describe("read-only restriction policy (#6190)", () => {
  const prod = makeTarget({ name: "prod", baseUrl: "https://x", restrictions: ["read-only"] });
  const dev = makeTarget({ name: "dev", baseUrl: "https://x" });

  test("classifies click/type as mutating; navigate/assert as read-only", () => {
    expect(isMutatingStepKind("click")).toBe(true);
    expect(isMutatingStepKind("type")).toBe(true);
    expect(isMutatingStepKind("navigate")).toBe(false);
    expect(isMutatingStepKind("assert")).toBe(false);
    expect(isMutatingStepKind("wait-for")).toBe(false);
    expect(isMutatingStepKind("screenshot")).toBe(false);
  });

  test("a mutating step against a read-only target is REFUSED with an honest reason", () => {
    const decision = checkStepAllowed(prod, "click");
    expect(decision.allowed).toBe(false);
    if (!decision.allowed) {
      expect(decision.reason).toContain("read-only");
      expect(decision.reason).toContain("click");
    }
  });

  test("a read-only step against a read-only target is allowed", () => {
    expect(checkStepAllowed(prod, "navigate").allowed).toBe(true);
    expect(checkStepAllowed(prod, "assert").allowed).toBe(true);
  });

  test("any step against a writable target is allowed", () => {
    expect(checkStepAllowed(dev, "click").allowed).toBe(true);
    expect(checkStepAllowed(dev, "type").allowed).toBe(true);
  });
});
