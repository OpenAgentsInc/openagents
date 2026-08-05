import * as nip19 from "nostr-effect/nip19";
import { describe, expect, test } from "vitest";

import {
  canonicalNostrPubkey,
  isGpt56AccountAllowed,
  isGpt56ModelId,
  isGpt56ReasoningEffort,
  parseGpt56AllowedNostrPubkeys,
} from "./gpt56-access";

const PUBKEY = "84b1282f95032fc0c45dd158d3f972eb60c007f42a1fa39b005fb950903819bf";

describe("GPT-5.6 access policy", () => {
  test("recognizes only the three exact hosted model ids", () => {
    expect(isGpt56ModelId("gpt-5.6-luna")).toBe(true);
    expect(isGpt56ModelId(" GPT-5.6-Terra ")).toBe(true);
    expect(isGpt56ModelId("gpt-5.6-sol")).toBe(true);
    expect(isGpt56ModelId("openai/gpt-5.6-sol")).toBe(false);
    expect(isGpt56ModelId("fireworks/gpt-5.6-sol")).toBe(false);
    expect(isGpt56ModelId("accounts/fireworks/models/gpt-5.6-sol")).toBe(false);
    expect(isGpt56ModelId("gpt-5.6")).toBe(false);
  });

  test("accepts the documented reasoning efforts only", () => {
    for (const effort of ["none", "low", "medium", "high", "xhigh", "max"]) {
      expect(isGpt56ReasoningEffort(effort)).toBe(true);
    }
    expect(isGpt56ReasoningEffort("minimal")).toBe(false);
    expect(isGpt56ReasoningEffort("MAX")).toBe(false);
  });

  test("canonicalizes hex and npub values into one public key set", () => {
    const npub = nip19.npubEncode(PUBKEY);
    expect(canonicalNostrPubkey(npub)).toBe(PUBKEY);
    expect(parseGpt56AllowedNostrPubkeys(`${npub}, ${PUBKEY}`)).toEqual(new Set([PUBKEY]));
  });

  test("fails the whole whitelist closed when any non-empty entry is invalid", () => {
    expect(parseGpt56AllowedNostrPubkeys(`${PUBKEY},not-an-npub`)).toEqual(new Set());
  });

  test("requires an exact verified Nostr OpenAuth subject and fails closed", () => {
    const allowed = parseGpt56AllowedNostrPubkeys(PUBKEY);
    expect(isGpt56AccountAllowed(`openauth:nostr:${PUBKEY}`, allowed)).toBe(true);
    expect(isGpt56AccountAllowed(`agent:nostr:${PUBKEY}`, allowed)).toBe(false);
    expect(isGpt56AccountAllowed("openauth:github:owner", allowed)).toBe(false);
    expect(isGpt56AccountAllowed(`openauth:nostr:${"a".repeat(64)}`, allowed)).toBe(false);
    expect(isGpt56AccountAllowed(`openauth:nostr:${PUBKEY}`, new Set())).toBe(false);
  });
});
