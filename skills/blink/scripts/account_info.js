#!/usr/bin/env node
/**
 * Blink Wallet - Account Info
 *
 * Usage: node account_info.js
 *
 * Shows account level, spending limits (withdrawal, internal send, convert),
 * default wallet, and wallet summary. Useful for understanding constraints
 * before attempting operations that might exceed limits.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const DEFAULT_API_URL = 'https://api.blink.sv/graphql';

function getApiKey() {
  let key = process.env.BLINK_API_KEY;
  if (!key) {
    try {
      const profile = fs.readFileSync(path.join(os.homedir(), '.profile'), 'utf8');
      const match = profile.match(/BLINK_API_KEY=["']?([a-zA-Z0-9_]+)["']?/);
      if (match) key = match[1];
    } catch {}
  }
  if (!key) throw new Error('BLINK_API_KEY not found. Set it in environment or ~/.profile');
  return key;
}

function getApiUrl() {
  return process.env.BLINK_API_URL || DEFAULT_API_URL;
}

async function graphqlRequest(query, variables = {}) {
  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  const res = await fetch(apiUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-API-KEY': apiKey,
    },
    body: JSON.stringify({ query, variables }),
  });

  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${await res.text()}`);
  }

  const json = await res.json();
  if (json.errors && json.errors.length > 0) {
    throw new Error(`GraphQL error: ${json.errors.map(e => e.message).join(', ')}`);
  }
  return json.data;
}

const ACCOUNT_INFO_QUERY = `
  query AccountInfo {
    me {
      id
      username
      defaultAccount {
        id
        level
        defaultWallet {
          id
          walletCurrency
        }
        wallets {
          id
          walletCurrency
          balance
          pendingIncomingBalance
        }
        limits {
          withdrawal {
            ... on OneDayAccountLimit {
              totalLimit
              remainingLimit
              interval
            }
          }
          internalSend {
            ... on OneDayAccountLimit {
              totalLimit
              remainingLimit
              interval
            }
          }
          convert {
            ... on OneDayAccountLimit {
              totalLimit
              remainingLimit
              interval
            }
          }
        }
      }
    }
  }
`;

// Public query — converts a fiat amount to sat/cent equivalents (no base/offset math)
const CONVERSION_QUERY = `
  query CurrencyConversion($amount: Float!, $currency: DisplayCurrency!) {
    currencyConversionEstimation(amount: $amount, currency: $currency) {
      btcSatAmount
      usdCentAmount
    }
  }
`;

// Estimate USD value of a sat amount using currencyConversionEstimation.
async function estimateSatsToUsd(sats) {
  if (sats === 0) return 0;
  try {
    const data = await graphqlRequest(CONVERSION_QUERY, { amount: 1.0, currency: 'USD' });
    const est = data.currencyConversionEstimation;
    if (!est || !est.btcSatAmount || est.btcSatAmount === 0) return null;
    const usdPerSat = 1.0 / est.btcSatAmount;
    return Math.round(sats * usdPerSat * 100) / 100;
  } catch {
    return null; // non-fatal — USD estimate is best-effort
  }
}

function formatLimit(limits) {
  if (!limits || limits.length === 0) return null;
  return limits.map(l => ({
    totalLimitCents: l.totalLimit,
    remainingLimitCents: l.remainingLimit,
    totalLimitUsd: `$${(l.totalLimit / 100).toFixed(2)}`,
    remainingLimitUsd: l.remainingLimit != null ? `$${(l.remainingLimit / 100).toFixed(2)}` : null,
    intervalSeconds: l.interval,
    intervalHours: l.interval ? Math.round(l.interval / 3600) : null,
  }));
}

async function main() {
  const data = await graphqlRequest(ACCOUNT_INFO_QUERY);
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');

  const me = data.me;
  const account = me.defaultAccount;

  const walletEntries = [];
  for (const w of account.wallets) {
    const entry = {
      id: w.id,
      currency: w.walletCurrency,
      balance: w.balance,
      pendingIncoming: w.pendingIncomingBalance,
      unit: w.walletCurrency === 'BTC' ? 'sats' : 'cents',
    };
    if (w.walletCurrency === 'BTC') {
      const usdEstimate = await estimateSatsToUsd(w.balance);
      if (usdEstimate !== null) {
        entry.balanceUsd = usdEstimate;
        entry.balanceUsdFormatted = `$${usdEstimate.toFixed(2)}`;
      }
    }
    if (w.walletCurrency === 'USD') {
      entry.balanceUsdFormatted = `$${(w.balance / 100).toFixed(2)}`;
    }
    walletEntries.push(entry);
  }

  const output = {
    userId: me.id,
    username: me.username || null,
    accountId: account.id,
    accountLevel: account.level,
    defaultWallet: {
      id: account.defaultWallet.id,
      currency: account.defaultWallet.walletCurrency,
    },
    wallets: walletEntries,
    limits: {
      withdrawal: formatLimit(account.limits.withdrawal),
      internalSend: formatLimit(account.limits.internalSend),
      convert: formatLimit(account.limits.convert),
    },
  };

  console.log(JSON.stringify(output, null, 2));
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});