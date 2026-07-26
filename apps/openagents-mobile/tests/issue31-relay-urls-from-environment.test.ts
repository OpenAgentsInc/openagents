/**
 * The issue #31 relay list is build-time configurable, and refuses junk whole.
 *
 * Added during the omega#49 device proof. The list was a compile-time constant,
 * so putting a phone and an Omega host on the same relay meant editing the
 * source — which means the binary that was proven is not the binary that ships.
 *
 * The refusal rule is the part worth pinning: a malformed override falls back
 * to the shipped default set in full, rather than reading whichever entries
 * happened to parse. A client silently reading half the relays it was told to
 * read reports gaps and coverage against a set nobody chose.
 *
 * This is deliberately not a runtime setting, and no test here should be read
 * as inviting one. A relay the user can retarget from inside the app is a relay
 * an attacker can retarget, and the client's admission model assumes its relay
 * list came from out of band.
 */
import { describe, expect, test } from "vite-plus/test";

import {
  OPENAGENTS_ISSUE31_RELAY_URLS,
  issue31RelayUrlsFromEnvironment,
} from "../src/workroom/issue31-mobile-nostr-runtime.ts";

const shipped = [...OPENAGENTS_ISSUE31_RELAY_URLS];

describe("issue 31 relay urls from environment", () => {
  test("falls back to the shipped set when unset or empty", () => {
    expect([...issue31RelayUrlsFromEnvironment(undefined)]).toEqual(shipped);
    expect([...issue31RelayUrlsFromEnvironment("")]).toEqual(shipped);
    expect([...issue31RelayUrlsFromEnvironment("   ")]).toEqual(shipped);
  });

  test("accepts a single relay", () => {
    expect([...issue31RelayUrlsFromEnvironment("wss://relay.openagents.com")]).toEqual([
      "wss://relay.openagents.com",
    ]);
  });

  test("accepts a comma list, trims it, and drops duplicates", () => {
    expect([
      ...issue31RelayUrlsFromEnvironment(" wss://one.example , wss://two.example ,wss://one.example"),
    ]).toEqual(["wss://one.example", "wss://two.example"]);
  });

  test("accepts ws:// for a local relay", () => {
    expect([...issue31RelayUrlsFromEnvironment("ws://127.0.0.1:32001")]).toEqual([
      "ws://127.0.0.1:32001",
    ]);
  });

  test("refuses the whole override when any entry is malformed", () => {
    // The good entry must not survive on its own.
    for (const value of [
      "wss://good.example,https://bad.example",
      "wss://good.example,not-a-url",
      "wss://good.example,wss://has space.example",
    ]) {
      expect([...issue31RelayUrlsFromEnvironment(value)]).toEqual(shipped);
    }
  });

  test("refuses a URL carrying credentials, a query, or a fragment", () => {
    for (const value of [
      "wss://owner:secret@relay.example",
      "wss://relay.example/?token=secret",
      "wss://relay.example/#fragment",
    ]) {
      expect([...issue31RelayUrlsFromEnvironment(value)]).toEqual(shipped);
    }
  });

  test("refuses more relays than the client accepts", () => {
    const nine = Array.from({ length: 9 }, (_, index) => `wss://relay${index}.example`).join(",");
    expect([...issue31RelayUrlsFromEnvironment(nine)]).toEqual(shipped);
    const eight = Array.from({ length: 8 }, (_, index) => `wss://relay${index}.example`).join(",");
    expect(issue31RelayUrlsFromEnvironment(eight)).toHaveLength(8);
  });
});
