import { describe, expect, test } from "vite-plus/test";

import {
  DESTINATION_FAILURE_MESSAGES,
  ENTRY_NOTICE_MESSAGES,
  SWP_DESTINATION_ERROR_MESSAGES,
} from "./messages.js";
import {
  DESTINATION_PARSE_FAILURE_MODES,
  type DestinationParseFailureMode,
} from "./model.js";
import { parseDestination, type DestinationParseContext } from "./parse.js";
import {
  DEFAULT_TEST_TIMESTAMP,
  encodeTestBase58Address,
  encodeTestInvoice,
  encodeTestLnurl,
  encodeTestSegwitAddress,
} from "./testkit.js";

const NOW = DEFAULT_TEST_TIMESTAMP + 60;

const chainRegtest: DestinationParseContext = {
  rail: "chain",
  network: "regtest",
  nowSeconds: NOW,
};
const lightningRegtest: DestinationParseContext = {
  rail: "lightning",
  network: "regtest",
  nowSeconds: NOW,
};
const chainMainnet: DestinationParseContext = {
  rail: "chain",
  network: "mainnet",
  nowSeconds: NOW,
};

const expectFailure = (
  input: string,
  context: DestinationParseContext,
  mode: DestinationParseFailureMode,
) => {
  const result = parseDestination(input, context);
  expect(result.ok).toBe(false);
  if (!result.ok) expect(result.failure.mode).toBe(mode);
  return result.ok ? null : result.failure;
};

const mutate = (s: string): string =>
  s.slice(0, -1) + (s.endsWith("q") ? "p" : "q");

describe("parseDestination failure modes (negative-case table)", () => {
  const table: readonly {
    readonly name: string;
    readonly input: string;
    readonly context: DestinationParseContext;
    readonly mode: DestinationParseFailureMode;
  }[] = [
    { name: "empty string", input: "", context: chainRegtest, mode: "empty" },
    {
      name: "whitespace only",
      input: "   ",
      context: chainRegtest,
      mode: "empty",
    },
    {
      name: "bare EVM address",
      input: "0x52908400098527886E0F7030069857D2E4169EE7",
      context: chainRegtest,
      mode: "evm_destination_unsupported",
    },
    {
      name: "ethereum URI",
      input: "ethereum:0x52908400098527886E0F7030069857D2E4169EE7",
      context: chainRegtest,
      mode: "evm_destination_unsupported",
    },
    {
      name: "http URL",
      input: "https://example.com/pay",
      context: chainRegtest,
      mode: "uri_scheme_unsupported",
    },
    {
      name: "foreign chain URI",
      input: "litecoin:LcHKx9DiBEmStl4hDzMz4iYZDfmM7t3S3D",
      context: chainRegtest,
      mode: "uri_scheme_unsupported",
    },
    {
      name: "BIP-21 unknown required parameter",
      input: `bitcoin:${encodeTestSegwitAddress("regtest", 0, 20)}?req-somethingelse=1`,
      context: chainRegtest,
      mode: "uri_required_param_unsupported",
    },
    {
      name: "bech32 address with flipped character",
      input: mutate(encodeTestSegwitAddress("regtest", 0, 20)),
      context: chainRegtest,
      mode: "address_checksum_invalid",
    },
    {
      name: "mixed-case bech32 address",
      input: (() => {
        const a = encodeTestSegwitAddress("regtest", 0, 20);
        return a.slice(0, 6) + a.slice(6).toUpperCase();
      })(),
      context: chainRegtest,
      mode: "address_checksum_invalid",
    },
    {
      name: "base58 address with corrupted checksum",
      input: mutate(encodeTestBase58Address(0x00)) + "1",
      context: chainMainnet,
      mode: "address_checksum_invalid",
    },
    {
      name: "witness v0 encoded with bech32m",
      input: encodeTestSegwitAddress("regtest", 0, 20, 0x05, "bech32m"),
      context: chainRegtest,
      mode: "address_encoding_mismatch",
    },
    {
      name: "witness v1 encoded with bech32",
      input: encodeTestSegwitAddress("regtest", 1, 32, 0x05, "bech32"),
      context: chainRegtest,
      mode: "address_encoding_mismatch",
    },
    {
      name: "witness v0 with 25-byte program",
      input: encodeTestSegwitAddress("regtest", 0, 25),
      context: chainRegtest,
      mode: "address_witness_program_invalid",
    },
    {
      name: "witness v1 with 20-byte program",
      input: encodeTestSegwitAddress("regtest", 1, 20),
      context: chainRegtest,
      mode: "address_witness_program_invalid",
    },
    {
      name: "witness v2 address",
      input: encodeTestSegwitAddress("regtest", 2, 32),
      context: chainRegtest,
      mode: "address_witness_program_invalid",
    },
    {
      name: "base58 address with unknown version byte",
      input: encodeTestBase58Address(0x33),
      context: chainMainnet,
      mode: "address_base58_version_unknown",
    },
    {
      name: "mainnet address on a regtest pair",
      input: encodeTestSegwitAddress("mainnet", 0, 20),
      context: chainRegtest,
      mode: "address_network_mismatch",
    },
    {
      name: "regtest address on a mainnet pair",
      input: encodeTestSegwitAddress("regtest", 0, 20),
      context: chainMainnet,
      mode: "address_network_mismatch",
    },
    {
      name: "base58 testnet address on a mainnet pair",
      input: encodeTestBase58Address(0x6f),
      context: chainMainnet,
      mode: "address_network_mismatch",
    },
    {
      name: "invoice with flipped character",
      input: mutate(encodeTestInvoice()),
      context: lightningRegtest,
      mode: "invoice_checksum_invalid",
    },
    {
      name: "invoice without a payment hash",
      input: encodeTestInvoice({ omitPaymentHash: true }),
      context: lightningRegtest,
      mode: "invoice_malformed",
    },
    {
      name: "invoice without a description commitment",
      input: encodeTestInvoice({ omitDescription: true }),
      context: lightningRegtest,
      mode: "invoice_malformed",
    },
    {
      name: "invoice with pico amount not a multiple of 10",
      input: encodeTestInvoice({ amount: "2501p" }),
      context: lightningRegtest,
      mode: "invoice_amount_invalid",
    },
    {
      name: "invoice with zero amount digits",
      input: encodeTestInvoice({ amount: "0" }),
      context: lightningRegtest,
      mode: "invoice_amount_invalid",
    },
    {
      name: "invoice with leading-zero amount",
      input: encodeTestInvoice({ amount: "0500n" }),
      context: lightningRegtest,
      mode: "invoice_amount_invalid",
    },
    {
      name: "mainnet invoice on a regtest pair",
      input: encodeTestInvoice({ network: "mainnet" }),
      context: lightningRegtest,
      mode: "invoice_network_mismatch",
    },
    {
      name: "expired invoice",
      input: encodeTestInvoice({
        timestampSeconds: DEFAULT_TEST_TIMESTAMP - 7200,
        expirySeconds: 60,
      }),
      context: lightningRegtest,
      mode: "invoice_expired",
    },
    {
      name: "amountless invoice",
      input: encodeTestInvoice({ amount: null }),
      context: lightningRegtest,
      mode: "invoice_amountless",
    },
    {
      // Last-field-wins would let one invoice carry two expiry readings.
      name: "invoice with a duplicate expiry field",
      input: encodeTestInvoice({
        expirySeconds: 3600,
        extraTaggedFields: [{ type: 6, data: [1] }],
      }),
      context: lightningRegtest,
      mode: "invoice_malformed",
    },
    {
      name: "invoice with a duplicate min-final-cltv field",
      input: encodeTestInvoice({
        minFinalCltv: 18,
        extraTaggedFields: [{ type: 24, data: [1] }],
      }),
      context: lightningRegtest,
      mode: "invoice_malformed",
    },
    {
      // 12 words = 60 bits: past MAX_SAFE_INTEGER, where an unbounded
      // parser silently rounds and diverges from the engine's reading.
      name: "invoice with an out-of-range expiry value",
      input: encodeTestInvoice({
        extraTaggedFields: [{ type: 6, data: Array.from({ length: 12 }, () => 31) }],
      }),
      context: lightningRegtest,
      mode: "invoice_malformed",
    },
    {
      name: "lnurl with corrupted payload",
      input: mutate(encodeTestLnurl("https://pay.example.com/lnurlp/alice")),
      context: lightningRegtest,
      mode: "lnurl_malformed",
    },
    {
      name: "lnurl wrapping a non-http url",
      input: encodeTestLnurl("ftp://pay.example.com/x"),
      context: lightningRegtest,
      mode: "lnurl_malformed",
    },
    {
      name: "bolt12 offer with invalid charset",
      input: "lno1bbbbbbbbbbbbbbbbbbbb",
      context: lightningRegtest,
      mode: "offer_malformed",
    },
    {
      name: "arbitrary text",
      input: "not a destination at all",
      context: chainRegtest,
      mode: "unrecognized",
    },
  ];

  for (const row of table) {
    test(row.name, () => {
      expectFailure(row.input, row.context, row.mode);
    });
  }

  test("the negative table covers most of the mode set", () => {
    const covered = new Set(table.map((row) => row.mode));
    // `route_unreachable` is produced by the entry reducer, covered in
    // entry.test.ts.
    const parserModes = DESTINATION_PARSE_FAILURE_MODES.filter(
      (mode) => mode !== "route_unreachable",
    );
    for (const mode of parserModes) {
      expect(covered.has(mode), `mode ${mode} lacks a negative case`).toBe(true);
    }
  });

  test("an amountless invoice is refused with the typed identifier", () => {
    const failure = expectFailure(
      encodeTestInvoice({ amount: null }),
      lightningRegtest,
      "invoice_amountless",
    );
    expect(failure?.swpError).toBe("swp_invoice_invalid");
  });
});

describe("no two failure modes collapse into one message", () => {
  test("every mode has an entry with a distinct message and key", () => {
    const entries = DESTINATION_PARSE_FAILURE_MODES.map(
      (mode) => DESTINATION_FAILURE_MESSAGES[mode],
    );
    expect(entries).toHaveLength(DESTINATION_PARSE_FAILURE_MODES.length);
    const keys = new Set(entries.map((entry) => entry.key));
    const messages = new Set(entries.map((entry) => entry.message));
    expect(keys.size).toBe(entries.length);
    expect(messages.size).toBe(entries.length);
  });

  test("notice and verdict messages are distinct too", () => {
    const all = [
      ...Object.values(DESTINATION_FAILURE_MESSAGES),
      ...Object.values(ENTRY_NOTICE_MESSAGES),
      ...Object.values(SWP_DESTINATION_ERROR_MESSAGES),
    ];
    expect(new Set(all.map((entry) => entry.key)).size).toBe(all.length);
    expect(new Set(all.map((entry) => entry.message)).size).toBe(all.length);
  });
});

describe("parseDestination successes", () => {
  test("regtest p2wpkh binds on the chain rail", () => {
    const address = encodeTestSegwitAddress("regtest", 0, 20);
    const result = parseDestination(address, chainRegtest);
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "onchain_address") {
      expect(result.destination.addressType).toBe("p2wpkh");
      expect(result.destination.networkClass).toBe("regtest");
      expect(result.requiredRail).toBeNull();
    } else {
      throw new Error("expected onchain address");
    }
  });

  test("tb1 addresses satisfy both testnet and signet pairs", () => {
    const address = encodeTestSegwitAddress("testnet", 0, 20);
    for (const network of ["testnet", "signet"] as const) {
      const result = parseDestination(address, {
        rail: "chain",
        network,
        nowSeconds: NOW,
      });
      expect(result.ok).toBe(true);
    }
  });

  test("base58 mainnet p2pkh and p2sh classify", () => {
    for (const [version, type] of [
      [0x00, "p2pkh"],
      [0x05, "p2sh"],
    ] as const) {
      const result = parseDestination(
        encodeTestBase58Address(version),
        chainMainnet,
      );
      expect(result.ok).toBe(true);
      if (result.ok && result.destination.kind === "onchain_address") {
        expect(result.destination.addressType).toBe(type);
      }
    }
  });

  test("p2tr (v1, 32 bytes) classifies", () => {
    const result = parseDestination(
      encodeTestSegwitAddress("regtest", 1, 32),
      chainRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "onchain_address") {
      expect(result.destination.addressType).toBe("p2tr");
    }
  });

  test("uppercase QR form of a bech32 address parses", () => {
    const address = encodeTestSegwitAddress("regtest", 0, 20).toUpperCase();
    const result = parseDestination(address, chainRegtest);
    expect(result.ok).toBe(true);
  });

  test("an invoice surfaces amount, expiry, cltv, hints, and commitment", () => {
    const invoice = encodeTestInvoice({
      amount: "2500u",
      expirySeconds: 7200,
      minFinalCltv: 40,
      routeHints: 2,
      includePaymentSecret: true,
    });
    const result = parseDestination(invoice, lightningRegtest);
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "bolt11_invoice") {
      // 2500u = 0.0025 BTC = 250 000 sat = 250 000 000 msat.
      expect(result.destination.amountMsat).toBe(250_000_000n);
      expect(result.destination.expirySeconds).toBe(7200);
      expect(result.destination.expiresAtSeconds).toBe(
        DEFAULT_TEST_TIMESTAMP + 7200,
      );
      expect(result.destination.minFinalCltvExpiryDelta).toBe(40);
      expect(result.destination.routeHintCount).toBe(2);
      expect(result.destination.hasPaymentSecret).toBe(true);
      expect(result.destination.paymentHashHex).toBe("01".repeat(32));
      expect(result.destination.description).toEqual({
        kind: "description",
        text: "1 cup coffee",
      });
    } else {
      throw new Error("expected invoice");
    }
  });

  test("BOLT11 defaults apply: expiry 3600, cltv 18", () => {
    const result = parseDestination(encodeTestInvoice(), lightningRegtest);
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "bolt11_invoice") {
      expect(result.destination.expirySeconds).toBe(3600);
      expect(result.destination.minFinalCltvExpiryDelta).toBe(18);
    }
  });

  test("description-hash commitment survives", () => {
    const result = parseDestination(
      encodeTestInvoice({ descriptionHashByte: 0x0a }),
      lightningRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "bolt11_invoice") {
      expect(result.destination.description).toEqual({
        kind: "description_hash",
        hashHex: "0a".repeat(32),
      });
    }
  });

  test("lightning: URI unwraps to the same parse", () => {
    const invoice = encodeTestInvoice();
    const result = parseDestination(`lightning:${invoice}`, lightningRegtest);
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.destination.kind).toBe("bolt11_invoice");
  });

  test("lightning address resolves to a LUD-16 URL", () => {
    const result = parseDestination("Alice@Pay.Example.COM", lightningRegtest);
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "deferred_lightning_address") {
      expect(result.destination.address).toBe("Alice@pay.example.com");
      expect(result.destination.resolveUrl).toBe(
        "https://pay.example.com/.well-known/lnurlp/Alice",
      );
    } else {
      throw new Error("expected lightning address");
    }
  });

  test("lnurl decodes to its URL", () => {
    const result = parseDestination(
      encodeTestLnurl("https://pay.example.com/lnurlp/alice"),
      lightningRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "deferred_lnurl") {
      expect(result.destination.url).toBe(
        "https://pay.example.com/lnurlp/alice",
      );
    }
  });

  test("a bolt12 offer is recognised as deferred", () => {
    const result = parseDestination(
      "lno1qgsqvgnwgcg35z6ee2h3yczraddm72xrfua9uve2rlrm9deu7xyfzrcgq",
      lightningRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.destination.kind).toBe("deferred_bolt12_offer");
  });

  test("pasting an invoice on a chain field asks for the lightning rail", () => {
    const result = parseDestination(encodeTestInvoice(), chainRegtest);
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.requiredRail).toBe("lightning");
  });

  test("pasting an address on a lightning field asks for the chain rail", () => {
    const result = parseDestination(
      encodeTestSegwitAddress("regtest", 0, 20),
      lightningRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.requiredRail).toBe("chain");
  });
});

describe("BIP-21 and unified QR", () => {
  const address = encodeTestSegwitAddress("regtest", 0, 20);

  test("amount and label are carried; case-insensitive keys", () => {
    const result = parseDestination(
      `bitcoin:${address}?AMOUNT=0.00300000&LABEL=test`,
      chainRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "onchain_address") {
      expect(result.destination.bip21?.amountSats).toBe(300_000n);
      expect(result.destination.bip21?.label).toBe("test");
    } else {
      throw new Error("expected onchain address");
    }
  });

  test("a malformed BIP-21 amount is treated as absent, not an error", () => {
    const result = parseDestination(
      `bitcoin:${address}?amount=12,5`,
      chainRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "onchain_address") {
      expect(result.destination.bip21?.amountSats).toBeNull();
    }
  });

  test("unified QR: invoice amount suppresses the BIP-21 amount", () => {
    const invoice = encodeTestInvoice({ amount: "2500u" });
    const result = parseDestination(
      `bitcoin:${address}?amount=0.005&lightning=${invoice}`,
      chainRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "unified") {
      expect(result.destination.bip21AmountSuppressed).toBe(true);
      expect(result.destination.lightning?.kind).toBe("bolt11_invoice");
      expect(result.destination.onchain.bip21?.amountSats).toBe(500_000n);
    } else {
      throw new Error("expected unified destination");
    }
  });

  test("unified QR with a broken invoice keeps the discriminated failure", () => {
    const invoice = encodeTestInvoice({ amount: null });
    const result = parseDestination(
      `bitcoin:${address}?lightning=${invoice}`,
      chainRegtest,
    );
    expect(result.ok).toBe(true);
    if (result.ok && result.destination.kind === "unified") {
      expect(result.destination.lightning).toBeNull();
      expect(result.destination.lightningFailure?.mode).toBe(
        "invoice_amountless",
      );
      expect(result.destination.bip21AmountSuppressed).toBe(false);
    } else {
      throw new Error("expected unified destination");
    }
  });
});
