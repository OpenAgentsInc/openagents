const PREIMAGE_REGEX = /^[0-9a-f]+$/;

export const normalizePreimageHex = (value: string): string | null => {
  const normalized = value.trim().toLowerCase();
  if (!normalized) return null;
  if (!PREIMAGE_REGEX.test(normalized)) return null;
  return normalized;
};

export const formatPaymentProofReference = (preimageHex: string): string =>
  `lightning_preimage:${preimageHex.slice(0, 24)}`;
