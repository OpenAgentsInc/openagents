#!/usr/bin/env node
/**
 * Blink Wallet - Check Balances
 *
 * Usage: node balance.js
 *
 * Queries the Blink API for all wallet balances (BTC and USD).
 * Outputs JSON with wallet IDs, currencies, and balances.
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

const BALANCE_QUERY = `
  query Me {
    me {
      defaultAccount {
        wallets {
          id
          walletCurrency
          balance
          pendingIncomingBalance
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
// Strategy: ask the API "how many sats is $1 USD worth?", then derive the rate.
async function estimateSatsToUsd(sats) {
  if (sats === 0) return 0;
  try {
    const data = await graphqlRequest(CONVERSION_QUERY, { amount: 1.0, currency: 'USD' });
    const est = data.currencyConversionEstimation;
    // est.btcSatAmount = how many sats $1.00 buys
    if (!est || !est.btcSatAmount || est.btcSatAmount === 0) return null;
    const usdPerSat = 1.0 / est.btcSatAmount;
    return Math.round(sats * usdPerSat * 100) / 100;
  } catch {
    return null; // non-fatal — USD estimate is best-effort
  }
}

async function main() {
  const data = await graphqlRequest(BALANCE_QUERY);

  if (!data.me) {
    throw new Error('Authentication failed. Check your BLINK_API_KEY.');
  }

  const wallets = data.me.defaultAccount.wallets;
  const result = {
    wallets: wallets.map(w => ({
      id: w.id,
      currency: w.walletCurrency,
      balance: w.balance,
      pendingIncoming: w.pendingIncomingBalance,
      unit: w.walletCurrency === 'BTC' ? 'sats' : 'cents',
    })),
  };

  // Add convenience top-level fields
  const btc = wallets.find(w => w.walletCurrency === 'BTC');
  const usd = wallets.find(w => w.walletCurrency === 'USD');
  if (btc) {
    result.btcWalletId = btc.id;
    result.btcBalance = btc.balance;
    result.btcBalanceSats = btc.balance;

    // Pre-compute USD estimate so the agent doesn't have to do its own math
    const usdEstimate = await estimateSatsToUsd(btc.balance);
    if (usdEstimate !== null) {
      result.btcBalanceUsd = usdEstimate;
      result.btcBalanceUsdFormatted = `$${usdEstimate.toFixed(2)}`;
    }
  }
  if (usd) {
    result.usdWalletId = usd.id;
    result.usdBalance = usd.balance;
    result.usdBalanceCents = usd.balance;
    result.usdBalanceFormatted = `$${(usd.balance / 100).toFixed(2)}`;
  }

  console.log(JSON.stringify(result, null, 2));
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});