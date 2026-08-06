import { describe, expect, test } from "vite-plus/test";

import { makeCatalog, render, type MessageKey } from "./catalog.js";
import { en } from "./en.js";
import { swapErrorMessages, swpErrorMessageKey } from "./error-table.js";
import { messageForReportedError, messageForSwpError } from "./errors.js";
import { SWP_ERROR_IDENTIFIERS, isSwpErrorIdentifier } from "./identifiers.js";
import { DEFAULT_LOCALE, SUPPORTED_LOCALES, catalogFor, selectLocale } from "./locale.js";

const catalog = makeCatalog();

// Conditions observed on the rail or in recovery rather than produced by a
// verification verdict. Everything else in §17 is a refusal and must say so.
const NON_REFUSAL_IDENTIFIERS = new Set([
  "swp_replacement",
  "swp_reorg",
  "swp_refund_failed",
  "swp_coordinator_unavailable",
  "swp_unresolved_loss",
]);

describe("MKT-SWP §17 identifier vocabulary", () => {
  test("carries all 58 identifiers, unique, in protocol shape", () => {
    expect(SWP_ERROR_IDENTIFIERS.length).toBe(58);
    expect(new Set(SWP_ERROR_IDENTIFIERS).size).toBe(SWP_ERROR_IDENTIFIERS.length);
    for (const identifier of SWP_ERROR_IDENTIFIERS) {
      expect(identifier).toMatch(/^swp_[a-z0-9_]+$/);
    }
  });

  test("guard accepts every identifier and rejects non-members", () => {
    for (const identifier of SWP_ERROR_IDENTIFIERS) {
      expect(isSwpErrorIdentifier(identifier)).toBe(true);
    }
    expect(isSwpErrorIdentifier("swp_not_a_real_identifier")).toBe(false);
    expect(isSwpErrorIdentifier("")).toBe(false);
    expect(isSwpErrorIdentifier("SWP_INVALID_AMOUNT")).toBe(false);
  });
});

describe("error-identifier message table", () => {
  test("every identifier renders a non-empty message", () => {
    for (const identifier of SWP_ERROR_IDENTIFIERS) {
      const message = messageForSwpError(catalog, identifier);
      expect(message.length).toBeGreaterThan(0);
      expect(message.trim()).toBe(message);
    }
  });

  test("no message leaks the raw identifier or softens into a generic apology", () => {
    for (const identifier of SWP_ERROR_IDENTIFIERS) {
      const message = messageForSwpError(catalog, identifier).toLowerCase();
      expect(message).not.toContain(identifier);
      expect(message).not.toContain("something went wrong");
      expect(message).not.toContain("unknown error");
      expect(message).not.toContain("oops");
    }
  });

  test("every verification failure reads as a refusal with its reason", () => {
    for (const identifier of SWP_ERROR_IDENTIFIERS) {
      const message = messageForSwpError(catalog, identifier);
      if (NON_REFUSAL_IDENTIFIERS.has(identifier)) {
        expect(message.startsWith("Refused:")).toBe(false);
      } else {
        expect(message.startsWith("Refused: ")).toBe(true);
        expect(message.length).toBeGreaterThan("Refused: ".length + 10);
      }
    }
  });

  test("table keys and identifier list agree exactly", () => {
    const tableKeys = new Set(Object.keys(swapErrorMessages));
    for (const identifier of SWP_ERROR_IDENTIFIERS) {
      expect(tableKeys.has(swpErrorMessageKey(identifier))).toBe(true);
    }
    // Every table entry is an identifier entry or the unrecognized fallback.
    expect(tableKeys.size).toBe(SWP_ERROR_IDENTIFIERS.length + 1);
  });
});

describe("untrusted error-report boundary", () => {
  test("a typed identifier resolves through the table", () => {
    expect(messageForReportedError(catalog, "swp_price_feed_invalid")).toBe(
      messageForSwpError(catalog, "swp_price_feed_invalid"),
    );
  });

  test("an unrecognized report renders the typed fallback, never the input", () => {
    const hostile = "swp_totally_fake <script>alert(1)</script> pay me first";
    const message = messageForReportedError(catalog, hostile);
    expect(message.length).toBeGreaterThan(0);
    expect(message).not.toContain("swp_totally_fake");
    expect(message).not.toContain("<script>");
    expect(message).toContain("does not recognize");
  });
});

describe("catalog back-fill", () => {
  test("a locale missing a key renders the source-language string, never blank", () => {
    const partial = makeCatalog({
      "swap.error.swp_quote_expired": "Abgelehnt: das Angebot ist abgelaufen.",
    });
    expect(partial["swap.error.swp_quote_expired"]).toBe("Abgelehnt: das Angebot ist abgelaufen.");
    for (const key of Object.keys(en) as ReadonlyArray<MessageKey>) {
      if (key === "swap.error.swp_quote_expired") continue;
      expect(partial[key]).toBe(catalog[key]);
    }
    for (const identifier of SWP_ERROR_IDENTIFIERS) {
      expect(messageForSwpError(partial, identifier).length).toBeGreaterThan(0);
    }
  });

  test("an override of a parameterised key keeps its typed parameters", () => {
    const partial = makeCatalog({
      "swap.refusal.below_minimum": (params) =>
        `Mindestens ${params.minimum} ${params.denomination}.`,
    });
    expect(
      render(partial, "swap.refusal.below_minimum", { minimum: "50 000", denomination: "sats" }),
    ).toBe("Mindestens 50 000 sats.");
  });
});

describe("parameterised messages", () => {
  test("the no-offerings label is distinct from pair loading", () => {
    expect(catalog["swap.widget.no_offerings"]).toBe("No providers are offering this pair.");
    expect(catalog["swap.widget.no_offerings"]).not.toBe(catalog["swap.widget.pairs_loading"]);
  });

  test("amount refusals state the limit in the user's current units", () => {
    const below = render(catalog, "swap.refusal.below_minimum", {
      minimum: "50 000",
      denomination: "sats",
    });
    expect(below).toContain("50 000");
    expect(below).toContain("sats");

    const above = render(catalog, "swap.refusal.above_maximum", {
      maximum: "0.1",
      denomination: "BTC",
    });
    expect(above).toContain("0.1");
    expect(above).toContain("BTC");

    const range = render(catalog, "swap.refusal.amount_range", {
      minimum: "0.0005",
      maximum: "0.1",
      denomination: "BTC",
    });
    expect(range).toContain("0.0005");
    expect(range).toContain("0.1");
    expect(range).toContain("BTC");
  });

  test("static keys render without parameters", () => {
    expect(render(catalog, "swap.error.swp_funding_not_authorized")).toContain("Refused:");
    expect(render(catalog, "swap.error.swp_liquid_output_invalid")).toContain("Liquid");
  });
});

describe("typed keys", () => {
  test("a removed or renamed key fails typecheck", () => {
    // @ts-expect-error — not a key of the source catalog.
    const out: unknown = render(catalog, "swap.error.swp_no_such_identifier");
    // The compile error above is the guard; at runtime the key is absent.
    expect(out).toBeUndefined();
  });

  test("a parameterised key without its parameters fails typecheck", () => {
    // @ts-expect-error — below_minimum requires MinimumAmountParams.
    expect(() => render(catalog, "swap.refusal.below_minimum")).toThrow();
  });
});

describe("locale selection precedence", () => {
  test("english is the only supported locale and the default", () => {
    expect(SUPPORTED_LOCALES).toEqual(["en"]);
    expect(DEFAULT_LOCALE).toBe("en");
    expect(catalogFor("en")["swap.error.swp_unresolved_loss"].length).toBeGreaterThan(0);
  });

  test("explicit setting wins, then url, then previous url, then browser, then default", () => {
    expect(selectLocale()).toBe("en");
    expect(selectLocale({ explicitSetting: "en", urlParam: "xx" })).toBe("en");
    expect(selectLocale({ explicitSetting: "xx", urlParam: "en" })).toBe("en");
    expect(selectLocale({ urlParam: "xx", previousUrlLocale: "en" })).toBe("en");
    expect(selectLocale({ browserLanguage: "en-GB" })).toBe("en");
    expect(selectLocale({ explicitSetting: "xx", urlParam: "yy", browserLanguage: "zz" })).toBe(
      "en",
    );
  });
});
