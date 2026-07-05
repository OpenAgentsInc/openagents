const ON_TOKENS = new Set(['1', 'true', 'yes', 'on'])

export const isKhalaCodeLightningPaymentsEnabled = (
  value: string | undefined,
): boolean =>
  typeof value === 'string' && ON_TOKENS.has(value.trim().toLowerCase())
