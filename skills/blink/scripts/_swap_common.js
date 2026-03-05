/**
 * Blink swap helpers for BTC <-> USD internal conversions.
 *
 * This module provides deterministic, machine-parseable swap quote estimation
 * and execution using Blink's intra-ledger transfer mutations.
 *
 * Originally authored by @AtlantisPleb (Christopher David) for the OpenAgents
 * project (https://github.com/OpenAgentsInc/openagents) under the Apache
 * License 2.0. Ported to blink-skill with modifications for v1.0.0 CLI
 * integration. See NOTICE file for full attribution.
 *
 * Zero external dependencies — Node.js 18+ built-ins only.
 */

const {
  graphqlRequest,
  CONVERSION_QUERY,
  MUTATION_TIMEOUT_MS,
  getAllWallets,
} = require('./_blink_client');

// ── Constants ────────────────────────────────────────────────────────────────

const DIRECTION_BTC_TO_USD = 'BTC_TO_USD';
const DIRECTION_USD_TO_BTC = 'USD_TO_BTC';

// ── GraphQL mutations ────────────────────────────────────────────────────────

const SATS_TO_USD_MUTATION = `
  mutation IntraLedgerPaymentSend($input: IntraLedgerPaymentSendInput!) {
    intraLedgerPaymentSend(input: $input) {
      status
      errors {
        code
        message
        path
      }
      transaction {
        id
      }
    }
  }
`;

const USD_TO_SATS_MUTATION = `
  mutation IntraLedgerUsdPaymentSend($input: IntraLedgerUsdPaymentSendInput!) {
    intraLedgerUsdPaymentSend(input: $input) {
      status
      errors {
        code
        message
        path
      }
      transaction {
        id
      }
    }
  }
`;

// ── Direction / unit parsing ─────────────────────────────────────────────────

/**
 * Normalise a user-supplied direction string.
 * Accepts: btc-to-usd, sell-btc, buy-usd, usd-to-btc, sell-usd, buy-btc.
 * @param {string} value
 * @returns {string|null}
 */
function normalizeDirection(value) {
  if (!value) return null;
  const normalized = value.trim().toLowerCase().replace(/_/g, '-');
  if (normalized === 'btc-to-usd' || normalized === 'sell-btc' || normalized === 'buy-usd') {
    return DIRECTION_BTC_TO_USD;
  }
  if (normalized === 'usd-to-btc' || normalized === 'sell-usd' || normalized === 'buy-btc') {
    return DIRECTION_USD_TO_BTC;
  }
  return null;
}

/**
 * Normalise a unit string to 'sats' or 'cents'.
 * @param {string} value
 * @returns {string|null}
 */
function parseUnit(value) {
  if (!value) return null;
  const normalized = value.trim().toLowerCase();
  if (normalized === 'sat' || normalized === 'sats' || normalized === 'satoshi') {
    return 'sats';
  }
  if (normalized === 'cent' || normalized === 'cents' || normalized === 'usd-cents') {
    return 'cents';
  }
  return null;
}

/**
 * Default unit for a given direction.
 * @param {string} direction
 * @returns {string}
 */
function defaultUnitForDirection(direction) {
  return direction === DIRECTION_BTC_TO_USD ? 'sats' : 'cents';
}

/**
 * Parse a string as a positive integer or throw.
 * @param {string} raw
 * @param {string} fieldName
 * @returns {number}
 */
function parsePositiveInt(raw, fieldName) {
  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${fieldName} must be a positive integer`);
  }
  return parsed;
}

// ── Arg parsing ──────────────────────────────────────────────────────────────

/**
 * Parse swap-specific CLI arguments from an argv array.
 *
 * @param {string[]} argv  Typically `process.argv.slice(2)`.
 * @returns {object} Parsed args or `{ error }` on invalid input.
 */
function parseCommonSwapArgs(argv) {
  if (argv.length < 2) {
    return {
      error:
        'Usage: node <script> <direction> <amount> [--unit sats|cents] [--ttl-seconds N] [--immediate] [--dry-run] [--memo "text"]',
    };
  }

  const direction = normalizeDirection(argv[0]);
  if (!direction) {
    return {
      error:
        'Invalid direction. Use btc-to-usd or usd-to-btc (aliases: sell-btc, buy-usd, sell-usd, buy-btc).',
    };
  }

  let unit = null;
  let ttlSeconds = 60;
  let immediateExecution = false;
  let dryRun = false;
  let memo = null;

  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--unit' && i + 1 < argv.length) {
      unit = parseUnit(argv[i + 1]);
      if (!unit) {
        return { error: 'Invalid --unit value. Use sats or cents.' };
      }
      i += 1;
      continue;
    }
    if (arg === '--ttl-seconds' && i + 1 < argv.length) {
      try {
        ttlSeconds = parsePositiveInt(argv[i + 1], 'ttl-seconds');
      } catch (error) {
        return { error: error.message };
      }
      i += 1;
      continue;
    }
    if (arg === '--immediate') {
      immediateExecution = true;
      continue;
    }
    if (arg === '--dry-run') {
      dryRun = true;
      continue;
    }
    if (arg === '--memo' && i + 1 < argv.length) {
      memo = argv[i + 1];
      i += 1;
      continue;
    }
    return { error: `Unknown argument: ${arg}` };
  }

  const amount = parsePositiveInt(argv[1], 'amount');
  const normalizedUnit = unit || defaultUnitForDirection(direction);

  return {
    direction,
    amount,
    unit: normalizedUnit,
    ttlSeconds,
    immediateExecution,
    dryRun,
    memo: memo && memo.trim() ? memo.trim() : null,
  };
}

// ── Wallet helpers ───────────────────────────────────────────────────────────

function assertSupportedDirectionUnit(direction, unit) {
  if (direction === DIRECTION_BTC_TO_USD && unit !== 'sats' && unit !== 'cents') {
    throw new Error(`Unsupported unit ${unit} for BTC->USD swap`);
  }
  if (direction === DIRECTION_USD_TO_BTC && unit !== 'cents' && unit !== 'sats') {
    throw new Error(`Unsupported unit ${unit} for USD->BTC swap`);
  }
}

/**
 * Resolve both BTC and USD wallets.
 * @param {object} opts
 * @param {string} opts.apiKey
 * @param {string} opts.apiUrl
 * @returns {{ btcWallet: object, usdWallet: object }}
 */
async function getWalletPair({ apiKey, apiUrl }) {
  const wallets = await getAllWallets({ apiKey, apiUrl });
  const btcWallet = wallets.find((w) => w.walletCurrency === 'BTC');
  const usdWallet = wallets.find((w) => w.walletCurrency === 'USD');
  if (!btcWallet || !usdWallet) {
    throw new Error('Both BTC and USD wallets are required for swap operations');
  }
  return { btcWallet, usdWallet };
}

// ── Estimation helpers ───────────────────────────────────────────────────────

async function estimateSatsForUsdAmount({ usdAmountFloat, apiKey, apiUrl }) {
  const data = await graphqlRequest({
    query: CONVERSION_QUERY,
    variables: { amount: usdAmountFloat, currency: 'USD' },
    apiKey,
    apiUrl,
  });
  const est = data.currencyConversionEstimation;
  if (!est || !Number.isFinite(est.btcSatAmount) || !Number.isFinite(est.usdCentAmount)) {
    throw new Error('Blink conversion estimate is unavailable');
  }
  return {
    sats: est.btcSatAmount,
    cents: est.usdCentAmount,
  };
}

async function estimateOneDollarRate({ apiKey, apiUrl }) {
  const oneDollar = await estimateSatsForUsdAmount({
    usdAmountFloat: 1.0,
    apiKey,
    apiUrl,
  });
  if (!oneDollar.sats || oneDollar.sats <= 0) {
    throw new Error('Invalid sats-per-dollar conversion rate from Blink');
  }
  return oneDollar.sats;
}

// ── Snapshot / delta ─────────────────────────────────────────────────────────

/**
 * Build a balance snapshot object from wallet pairs.
 * @param {object} btcWallet
 * @param {object} usdWallet
 * @returns {object}
 */
function walletSnapshot(btcWallet, usdWallet) {
  return {
    btcWalletId: btcWallet.id,
    usdWalletId: usdWallet.id,
    btcBalanceSats: btcWallet.balance,
    usdBalanceCents: usdWallet.balance,
    usdBalanceFormatted: `$${(usdWallet.balance / 100).toFixed(2)}`,
  };
}

/**
 * Compute the balance delta between pre- and post-swap snapshots.
 * @param {object} preBalance
 * @param {object} postBalance
 * @returns {{ btcDeltaSats: number, usdDeltaCents: number }}
 */
function computeBalanceDelta(preBalance, postBalance) {
  return {
    btcDeltaSats: postBalance.btcBalanceSats - preBalance.btcBalanceSats,
    usdDeltaCents: postBalance.usdBalanceCents - preBalance.usdBalanceCents,
  };
}

// ── Quote estimation ─────────────────────────────────────────────────────────

/**
 * Estimate a swap quote with current exchange rates and wallet balances.
 *
 * @param {object}  opts
 * @param {string}  opts.direction
 * @param {number}  opts.amount
 * @param {string}  opts.unit
 * @param {number}  opts.ttlSeconds
 * @param {boolean} opts.immediateExecution
 * @param {string}  opts.apiKey
 * @param {string}  opts.apiUrl
 * @returns {{ preBalance: object, quote: object }}
 */
async function estimateSwapQuote({
  direction,
  amount,
  unit,
  ttlSeconds,
  immediateExecution,
  apiKey,
  apiUrl,
}) {
  assertSupportedDirectionUnit(direction, unit);

  const { btcWallet, usdWallet } = await getWalletPair({ apiKey, apiUrl });
  const preBalance = walletSnapshot(btcWallet, usdWallet);
  const satsPerDollar = await estimateOneDollarRate({ apiKey, apiUrl });

  let amountInSats = null;
  let amountInCents = null;
  let amountOutSats = null;
  let amountOutCents = null;

  if (direction === DIRECTION_BTC_TO_USD) {
    if (unit === 'sats') {
      amountInSats = amount;
      amountOutCents = Math.max(1, Math.round((amountInSats / satsPerDollar) * 100));
    } else {
      amountOutCents = amount;
      const estimate = await estimateSatsForUsdAmount({
        usdAmountFloat: amountOutCents / 100,
        apiKey,
        apiUrl,
      });
      amountInSats = estimate.sats;
    }
  } else {
    if (unit === 'cents') {
      amountInCents = amount;
      const estimate = await estimateSatsForUsdAmount({
        usdAmountFloat: amountInCents / 100,
        apiKey,
        apiUrl,
      });
      amountOutSats = estimate.sats;
    } else {
      amountOutSats = amount;
      amountInCents = Math.max(1, Math.round((amountOutSats / satsPerDollar) * 100));
    }
  }

  // Clean up: only keep the relevant in/out for the direction
  if (direction === DIRECTION_BTC_TO_USD) {
    amountOutSats = null;
    amountInCents = null;
  } else {
    amountOutCents = null;
    amountInSats = null;
  }

  const nowEpochSeconds = Math.floor(Date.now() / 1000);
  const expiresAt = nowEpochSeconds + ttlSeconds;
  const quoteId = `blink-swap-${nowEpochSeconds}-${Math.floor(Math.random() * 1_000_000)}`;

  return {
    preBalance,
    quote: {
      quoteId,
      direction,
      requestedAmount: {
        value: amount,
        unit,
      },
      amountIn: direction === DIRECTION_BTC_TO_USD
        ? { value: amountInSats, unit: 'sats' }
        : { value: amountInCents, unit: 'cents' },
      amountOut: direction === DIRECTION_BTC_TO_USD
        ? { value: amountOutCents, unit: 'cents' }
        : { value: amountOutSats, unit: 'sats' },
      feeSats: 0,
      feeBps: 0,
      slippageBps: 0,
      immediateExecution,
      ttlSeconds,
      expiresAtEpochSeconds: expiresAt,
      rateSnapshot: {
        satsPerDollar,
      },
      quoteSource: 'blink:currencyConversionEstimation',
      executionPath:
        direction === DIRECTION_BTC_TO_USD
          ? 'blink:intraLedgerPaymentSend'
          : 'blink:intraLedgerUsdPaymentSend',
    },
  };
}

// ── Swap execution ───────────────────────────────────────────────────────────

/**
 * Execute a swap between BTC and USD wallets.
 *
 * @param {object}  opts
 * @param {string}  opts.direction
 * @param {object}  opts.quote       Quote object from estimateSwapQuote.
 * @param {string|null} opts.memo
 * @param {string}  opts.apiKey
 * @param {string}  opts.apiUrl
 * @returns {{ status: string, transactionId: string|null, preBalance: object, postBalance: object, balanceDelta: object }}
 */
async function executeSwap({
  direction,
  quote,
  memo,
  apiKey,
  apiUrl,
}) {
  const { btcWallet, usdWallet } = await getWalletPair({ apiKey, apiUrl });
  const preBalance = walletSnapshot(btcWallet, usdWallet);

  const senderWalletId = direction === DIRECTION_BTC_TO_USD ? btcWallet.id : usdWallet.id;
  const recipientWalletId = direction === DIRECTION_BTC_TO_USD ? usdWallet.id : btcWallet.id;
  const amount = quote.amountIn.value;

  const input = {
    walletId: senderWalletId,
    recipientWalletId,
    amount,
  };
  if (memo) input.memo = memo;

  const mutation = direction === DIRECTION_BTC_TO_USD ? SATS_TO_USD_MUTATION : USD_TO_SATS_MUTATION;
  const mutationKey = direction === DIRECTION_BTC_TO_USD ? 'intraLedgerPaymentSend' : 'intraLedgerUsdPaymentSend';

  const data = await graphqlRequest({
    query: mutation,
    variables: { input },
    apiKey,
    apiUrl,
    timeoutMs: MUTATION_TIMEOUT_MS,
  });
  const result = data[mutationKey];
  if (!result) {
    throw new Error('Blink swap mutation did not return a result');
  }

  if (result.errors && result.errors.length > 0) {
    const errMsg = result.errors.map((e) => `${e.message}${e.code ? ` [${e.code}]` : ''}`).join(', ');
    throw new Error(`Swap failed: ${errMsg}`);
  }

  // Re-fetch wallets to get post-swap balances
  const refreshed = await getWalletPair({ apiKey, apiUrl });
  const postBalance = walletSnapshot(refreshed.btcWallet, refreshed.usdWallet);

  return {
    status: result.status || 'UNKNOWN',
    transactionId: result.transaction ? result.transaction.id : null,
    preBalance,
    postBalance,
    balanceDelta: computeBalanceDelta(preBalance, postBalance),
  };
}

// ── Exports ──────────────────────────────────────────────────────────────────

module.exports = {
  // Constants
  DIRECTION_BTC_TO_USD,
  DIRECTION_USD_TO_BTC,
  SATS_TO_USD_MUTATION,
  USD_TO_SATS_MUTATION,

  // Parsing
  normalizeDirection,
  parseUnit,
  defaultUnitForDirection,
  parsePositiveInt,
  parseCommonSwapArgs,
  assertSupportedDirectionUnit,

  // Wallet
  getWalletPair,

  // Estimation
  estimateSatsForUsdAmount,
  estimateOneDollarRate,

  // Snapshot
  walletSnapshot,
  computeBalanceDelta,

  // Core
  estimateSwapQuote,
  executeSwap,
};
