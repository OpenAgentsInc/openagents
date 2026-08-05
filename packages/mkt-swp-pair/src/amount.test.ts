import { readFile } from "node:fs/promises";
import { describe, expect, test } from "vite-plus/test";

import {
  AMOUNT_PARSE_FAILURE_MODES,
  MAX_AMOUNT_SATS,
  formatAmountText,
  parseAmountText,
  satsFromWire,
  satsToWire,
  type AmountParseFailureMode,
  type DecimalSeparator,
  type Denomination,
} from "./amount.js";

describe("wire amount grammar (MKT-SWP §3.2)", () => {
  const accepted: readonly [string, bigint][] = [
    ["0", 0n],
    ["1", 1n],
    ["10000", 10_000n],
    ["2100000000000000", MAX_AMOUNT_SATS],
    ["18446744073709551615", 18_446_744_073_709_551_615n],
  ];
  for (const [wire, sats] of accepted) {
    test(`accepts ${wire}`, () => {
      expect(satsFromWire(wire)).toBe(sats);
      expect(satsToWire(sats)).toBe(wire);
    });
  }

  const refused: readonly string[] = [
    "",
    "01",
    "-1",
    "+1",
    "1.0",
    "1e8",
    "1 ",
    " 1",
    "1sats",
    "0x10",
    "18446744073709551616", // u64 + 1
    "1_000",
  ];
  for (const wire of refused) {
    test(`refuses ${JSON.stringify(wire)}`, () => {
      expect(satsFromWire(wire)).toBe(null);
    });
  }
});

describe("amount text parsing (exact bigint, negative-case table)", () => {
  const cases: readonly {
    readonly name: string;
    readonly text: string;
    readonly denomination: Denomination;
    readonly separator: DecimalSeparator;
    readonly expect: bigint | AmountParseFailureMode;
  }[] = [
    { name: "1 BTC", text: "1", denomination: "btc", separator: ".", expect: 100_000_000n },
    { name: "one satoshi in BTC", text: "0.00000001", denomination: "btc", separator: ".", expect: 1n },
    { name: "max supply in BTC", text: "21000000", denomination: "btc", separator: ".", expect: MAX_AMOUNT_SATS },
    { name: "max minus one sat in BTC", text: "20999999.99999999", denomination: "btc", separator: ".", expect: MAX_AMOUNT_SATS - 1n },
    { name: "comma separator", text: "0,0015", denomination: "btc", separator: ",", expect: 150_000n },
    { name: "bare fraction", text: ".5", denomination: "btc", separator: ".", expect: 50_000_000n },
    { name: "trailing separator", text: "2.", denomination: "btc", separator: ".", expect: 200_000_000n },
    { name: "sats integer", text: "12345", denomination: "sats", separator: ".", expect: 12_345n },
    { name: "max supply in sats", text: "2100000000000000", denomination: "sats", separator: ".", expect: MAX_AMOUNT_SATS },
    { name: "empty", text: "", denomination: "btc", separator: ".", expect: "empty" },
    { name: "whitespace only", text: "   ", denomination: "btc", separator: ".", expect: "empty" },
    { name: "letters", text: "abc", denomination: "btc", separator: ".", expect: "not_a_number" },
    { name: "wrong separator alone", text: "1,5", denomination: "btc", separator: ".", expect: "not_a_number" },
    { name: "two separators", text: "1.2.3", denomination: "btc", separator: ".", expect: "multiple_separators" },
    { name: "nine decimal places", text: "0.000000001", denomination: "btc", separator: ".", expect: "too_many_decimal_places" },
    { name: "sub-satoshi denied even when zero-padded", text: "0.000000010", denomination: "btc", separator: ".", expect: "too_many_decimal_places" },
    { name: "fractional sats", text: "1.5", denomination: "sats", separator: ".", expect: "sats_fractional" },
    { name: "comma in sats", text: "1,5", denomination: "sats", separator: ".", expect: "sats_fractional" },
    { name: "above supply in BTC", text: "21000000.00000001", denomination: "btc", separator: ".", expect: "exceeds_supply" },
    { name: "above supply in sats", text: "2100000000000001", denomination: "sats", separator: ".", expect: "exceeds_supply" },
    { name: "negative", text: "-1", denomination: "sats", separator: ".", expect: "not_a_number" },
    { name: "exponent", text: "1e8", denomination: "sats", separator: ".", expect: "not_a_number" },
  ];

  for (const c of cases) {
    test(c.name, () => {
      const result = parseAmountText(c.text, c.denomination, c.separator);
      if (typeof c.expect === "bigint") {
        expect(result).toEqual({ ok: true, sats: c.expect });
      } else {
        expect(result).toEqual({ ok: false, mode: c.expect });
      }
    });
  }

  test("every failure mode is in the exported mode set", () => {
    for (const c of cases) {
      if (typeof c.expect === "string") {
        expect(AMOUNT_PARSE_FAILURE_MODES).toContain(c.expect);
      }
    }
  });
});

describe("formatting round-trips are exact over atomic units", () => {
  const boundaries: readonly bigint[] = [
    0n,
    1n,
    99_999_999n,
    100_000_000n,
    100_000_001n,
    150_000n,
    2_099_999_999_999_999n,
    MAX_AMOUNT_SATS,
  ];
  for (const sats of boundaries) {
    for (const denomination of ["btc", "sats"] as const) {
      for (const separator of [".", ","] as const) {
        test(`${sats} sats round-trips as ${denomination} with "${separator}"`, () => {
          const text = formatAmountText(sats, denomination, separator);
          const parsed = parseAmountText(text, denomination, separator);
          expect(parsed).toEqual({ ok: true, sats });
        });
      }
    }
  }

  test("BTC formatting trims trailing zeros but keeps the whole part", () => {
    expect(formatAmountText(150_000n, "btc", ".")).toBe("0.0015");
    expect(formatAmountText(100_000_000n, "btc", ".")).toBe("1");
    expect(formatAmountText(1n, "btc", ".")).toBe("0.00000001");
    expect(formatAmountText(2_100_000_000_000_000n, "btc", ".")).toBe("21000000");
    expect(formatAmountText(123_456_789n, "btc", ",")).toBe("1,23456789");
  });

  test("sats formatting is the plain integer", () => {
    expect(formatAmountText(0n, "sats", ".")).toBe("0");
    expect(formatAmountText(MAX_AMOUNT_SATS, "sats", ".")).toBe("2100000000000000");
  });

  test("negative amounts throw instead of formatting", () => {
    expect(() => formatAmountText(-1n, "sats", ".")).toThrow(RangeError);
    expect(() => satsToWire(-1n)).toThrow(RangeError);
  });
});

describe("no floating-point path exists in the amount code", () => {
  test("amount.ts contains no float construct", async () => {
    const source = await readFile(
      new URL("./amount.ts", import.meta.url),
      "utf8",
    );
    for (const forbidden of [
      "parseFloat",
      "parseInt",
      "Number(",
      "Math.",
      "toFixed",
      "* 1e",
      "/ 1e",
    ]) {
      expect(source.includes(forbidden)).toBe(false);
    }
  });
});
