import * as nip19 from "nostr-effect/nip19";

import {
  GPT_56_LUNA_MODEL_ID,
  GPT_56_SOL_MODEL_ID,
  GPT_56_TERRA_MODEL_ID,
} from "./pricing";

export const GPT_56_REASONING_EFFORTS = ["none", "low", "medium", "high", "xhigh", "max"] as const;

export type Gpt56ReasoningEffort = (typeof GPT_56_REASONING_EFFORTS)[number];

const GPT_56_MODEL_IDS = new Set<string>([
  GPT_56_LUNA_MODEL_ID,
  GPT_56_TERRA_MODEL_ID,
  GPT_56_SOL_MODEL_ID,
]);

const HEX_PUBKEY_PATTERN = /^[0-9a-f]{64}$/u;

export const isGpt56ModelId = (modelId: string): boolean =>
  GPT_56_MODEL_IDS.has(modelId.trim().toLowerCase());

export const isGpt56ReasoningEffort = (value: unknown): value is Gpt56ReasoningEffort =>
  typeof value === "string" && (GPT_56_REASONING_EFFORTS as ReadonlyArray<string>).includes(value);

export const canonicalNostrPubkey = (value: string): string | undefined => {
  const normalized = value.trim().toLowerCase();
  if (HEX_PUBKEY_PATTERN.test(normalized)) {
    return normalized;
  }
  try {
    const decoded = nip19.decode(normalized);
    return decoded.type === "npub" &&
      typeof decoded.data === "string" &&
      HEX_PUBKEY_PATTERN.test(decoded.data)
      ? decoded.data
      : undefined;
  } catch {
    return undefined;
  }
};

export const parseGpt56AllowedNostrPubkeys = (
  configured: string | undefined,
): ReadonlySet<string> => {
  const entries = (configured ?? "")
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry !== "");
  const pubkeys = entries.map(canonicalNostrPubkey);
  return pubkeys.some((pubkey) => pubkey === undefined)
    ? new Set()
    : new Set(pubkeys.filter((pubkey): pubkey is string => pubkey !== undefined));
};

export const gpt56AccountNostrPubkey = (accountRef: string): string | undefined => {
  const prefix = "openauth:nostr:";
  if (!accountRef.startsWith(prefix)) {
    return undefined;
  }
  return canonicalNostrPubkey(accountRef.slice(prefix.length));
};

export const isGpt56AccountAllowed = (
  accountRef: string,
  allowedPubkeys: ReadonlySet<string>,
): boolean => {
  const pubkey = gpt56AccountNostrPubkey(accountRef);
  return pubkey !== undefined && allowedPubkeys.has(pubkey);
};
