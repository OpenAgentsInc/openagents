import { describe, expect, it } from "vite-plus/test";

import {
  callSites,
  compilerCommandFor,
  linkedProviderOf,
  macroValue,
  parseNmPerObject,
} from "./coldcard-build-driver.mjs";

/**
 * These are the measurement functions of the Coldcard artifact-witness driver.
 * Everything the capture claims about providers, call edges, macros, and the
 * symbol inventory is derived here from real toolchain output, so a silent
 * parsing mistake would become a silent false claim about firmware.
 */

describe("compilerCommandFor", () => {
  const log = [
    "arm-none-eabi-gcc -Wall -c -MD -o build-COLDCARD_MK4/rng.o rng.c",
    "arm-none-eabi-gcc -Wall -c -MD -o build-COLDCARD_MK4/boards/COLDCARD_MK4/rng.o boards/COLDCARD_MK4/rng.c",
  ].join("\n");

  it("matches the exact object path, not a suffix", () => {
    expect(compilerCommandFor(log, "build-COLDCARD_MK4/rng.o")).toContain(" rng.c");
    expect(compilerCommandFor(log, "build-COLDCARD_MK4/boards/COLDCARD_MK4/rng.o")).toContain(
      "boards/COLDCARD_MK4/rng.c",
    );
  });

  it("returns undefined when the translation unit was never compiled", () => {
    expect(compilerCommandFor(log, "build-COLDCARD_MK4/absent.o")).toBeUndefined();
  });
});

describe("macroValue", () => {
  it("reads the value the preprocessor actually used", () => {
    const dump = ["#define MICROPY_HW_ENABLE_RNG (0)", "#define OTHER 1"].join("\n");
    expect(macroValue(dump, "MICROPY_HW_ENABLE_RNG")).toBe("(0)");
  });

  it("does not invent a value for an undefined macro", () => {
    expect(macroValue("#define OTHER 1", "MICROPY_HW_ENABLE_RNG")).toBeUndefined();
  });
});

describe("callSites", () => {
  const dump = [
    "00000000 <random_uint32>:",
    "   0:\tb508      \tpush\t{r3, lr}",
    "   2:\tf7ff fffe \tbl\t0 <rng_get>",
    "\t\t\t2: R_ARM_THM_CALL\trng_get",
    "00000020 <dispatch>:",
    "  20:\t4790      \tblx\tr2",
  ].join("\n");

  it("attributes a relocation call site to its enclosing function", () => {
    expect(callSites(dump).edges).toEqual([{ from: "random_uint32", to: "rng_get" }]);
  });

  it("counts a register-indirect branch as unresolved rather than ignoring it", () => {
    expect(callSites(dump).unresolvedIndirect).toBe(1);
  });

  it("does not count a function return as an unresolved call site", () => {
    expect(callSites(["00000000 <f>:", "   0:\t4770      \tbx\tlr"].join("\n")).unresolvedIndirect).toBe(
      0,
    );
  });
});

describe("linkedProviderOf", () => {
  it("reads the object the link chose for a symbol", () => {
    const map = [
      " .text.rng_get  0x08059d08       0x8c build-COLDCARD_MK4/rng.o",
      "                0x08059d08                rng_get",
    ].join("\n");
    expect(linkedProviderOf(map, "rng_get")).toBe("build-COLDCARD_MK4/rng.o");
  });

  it("returns undefined when the symbol was not linked", () => {
    expect(linkedProviderOf(" .text.other 0x0 0x0 a.o", "rng_get")).toBeUndefined();
  });
});

describe("parseNmPerObject", () => {
  it("attributes every defined symbol to the object that defined it", () => {
    const text = [
      "build-COLDCARD_MK4/rng.o:",
      "00000000 T rng_get",
      "00000000 b seeded.4",
      "",
      "build-COLDCARD_MK4/boards/COLDCARD_MK4/rng.o:",
      "00000000 T random32",
    ].join("\n");
    expect(parseNmPerObject(text)).toEqual([
      { object: "build-COLDCARD_MK4/rng.o", type: "T", name: "rng_get" },
      { object: "build-COLDCARD_MK4/rng.o", type: "b", name: "seeded.4" },
      {
        object: "build-COLDCARD_MK4/boards/COLDCARD_MK4/rng.o",
        type: "T",
        name: "random32",
      },
    ]);
  });

  it("yields nothing for an object that defines nothing, which is the fixed build's upstream rng.o", () => {
    expect(parseNmPerObject("build-COLDCARD_MK4/rng.o:\n")).toEqual([]);
  });
});
