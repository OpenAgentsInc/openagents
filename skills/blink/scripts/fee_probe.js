#!/usr/bin/env node
/**
 * Blink Wallet - Fee Probe (Estimate Payment Fee)
 *
 * Usage: node fee_probe.js <bolt11_invoice>
 *
 * Estimates the fee for paying a Lightning invoice without actually sending.
 * Use this to check costs before committing to a payment.
 *
 * Arguments:
 *   bolt11_invoice  - Required. The BOLT-11 payment request string (lnbc...).
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

const WALLET_QUERY = `
  query Me {
    me {
      defaultAccount {
        wallets {
          id
          walletCurrency
          balance
        }
      }
    }
  }
`;

const FEE_PROBE_MUTATION = `
  mutation LnInvoiceFeeProbe($input: LnInvoiceFeeProbeInput!) {
    lnInvoiceFeeProbe(input: $input) {
      amount
      errors {
        code
        message
        path
      }
    }
  }
`;

async function getBtcWallet() {
  const data = await graphqlRequest(WALLET_QUERY);
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');
  const btcWallet = data.me.defaultAccount.wallets.find(w => w.walletCurrency === 'BTC');
  if (!btcWallet) throw new Error('No BTC wallet found on this account.');
  return btcWallet;
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error('Usage: node fee_probe.js <bolt11_invoice>');
    process.exit(1);
  }

  const paymentRequest = args[0].trim();
  if (!paymentRequest.startsWith('lnbc') && !paymentRequest.startsWith('lntbs') && !paymentRequest.startsWith('lntb')) {
    console.error('Warning: invoice does not start with lnbc/lntbs/lntb — may not be a valid BOLT-11 invoice.');
  }

  const btcWallet = await getBtcWallet();

  const input = {
    walletId: btcWallet.id,
    paymentRequest,
  };

  const data = await graphqlRequest(FEE_PROBE_MUTATION, { input });
  const result = data.lnInvoiceFeeProbe;

  if (result.errors && result.errors.length > 0) {
    const errMsg = result.errors.map(e => `${e.message}${e.code ? ` [${e.code}]` : ''}`).join(', ');
    throw new Error(`Fee probe failed: ${errMsg}`);
  }

  const output = {
    estimatedFeeSats: result.amount,
    walletId: btcWallet.id,
    walletBalance: btcWallet.balance,
  };

  if (result.amount === 0) {
    console.error('Fee estimate: 0 sats (internal transfer or direct channel)');
  } else {
    console.error(`Fee estimate: ${result.amount} sats`);
  }

  console.log(JSON.stringify(output, null, 2));
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});