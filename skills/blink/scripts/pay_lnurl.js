#!/usr/bin/env node
/**
 * Blink Wallet - Pay to LNURL
 *
 * Usage: node pay_lnurl.js <lnurl> <amount_sats> [--wallet BTC|USD]
 *
 * Sends satoshis to a raw LNURL payRequest string.
 * For Lightning Addresses (user@domain), use pay_lnaddress.js instead.
 * Automatically resolves the wallet ID from the account.
 *
 * When --wallet USD is used, the amount is still specified in satoshis.
 * The Blink API debits the USD equivalent from the USD wallet automatically.
 *
 * Arguments:
 *   lnurl        - Required. LNURL string (lnurl1...).
 *   amount_sats  - Required. Amount in satoshis to send.
 *   --wallet     - Optional. Wallet to pay from: BTC (default) or USD.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 *
 * CAUTION: This sends real bitcoin. The API key must have Write scope.
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

const PAY_LNURL_MUTATION = `
  mutation LnurlPaymentSend($input: LnurlPaymentSendInput!) {
    lnurlPaymentSend(input: $input) {
      status
      errors {
        code
        message
        path
      }
    }
  }
`;

async function getWallet(currency) {
  const data = await graphqlRequest(WALLET_QUERY);
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');
  const wallet = data.me.defaultAccount.wallets.find(w => w.walletCurrency === currency);
  if (!wallet) throw new Error(`No ${currency} wallet found on this account.`);
  return wallet;
}

function formatBalance(wallet) {
  if (wallet.walletCurrency === 'USD') {
    return `$${(wallet.balance / 100).toFixed(2)} (${wallet.balance} cents)`;
  }
  return `${wallet.balance} sats`;
}

function parseArgs(argv) {
  const args = { lnurl: null, amountSats: null, walletCurrency: 'BTC' };
  const raw = argv.slice(2);
  const positional = [];
  for (let i = 0; i < raw.length; i++) {
    if (raw[i] === '--wallet' && i + 1 < raw.length) {
      const val = raw[i + 1].toUpperCase();
      if (val !== 'BTC' && val !== 'USD') {
        console.error('Error: --wallet must be BTC or USD');
        process.exit(1);
      }
      args.walletCurrency = val;
      i++;
    } else {
      positional.push(raw[i]);
    }
  }
  if (positional.length >= 1) args.lnurl = positional[0].trim();
  if (positional.length >= 2) args.amountSats = parseInt(positional[1], 10);
  return args;
}

async function main() {
  const args = parseArgs(process.argv);
  if (!args.lnurl || args.amountSats === null) {
    console.error('Usage: node pay_lnurl.js <lnurl> <amount_sats> [--wallet BTC|USD]');
    process.exit(1);
  }

  const lnurl = args.lnurl;
  const amountSats = args.amountSats;

  if (!lnurl.toLowerCase().startsWith('lnurl')) {
    console.error('Warning: input does not start with "lnurl" \u2014 may not be a valid LNURL string.');
    console.error('For Lightning Addresses (user@domain), use pay_lnaddress.js instead.');
  }

  if (isNaN(amountSats) || amountSats <= 0) {
    console.error('Error: amount_sats must be a positive integer');
    process.exit(1);
  }

  // Resolve wallet (BTC or USD)
  const wallet = await getWallet(args.walletCurrency);

  // Balance warning (BTC balance is in sats, directly comparable; USD balance
  // is in cents, not directly comparable to sats — skip for USD)
  if (args.walletCurrency === 'BTC' && wallet.balance < amountSats) {
    console.error(`Warning: wallet balance (${wallet.balance} sats) may be insufficient for ${amountSats} sats + fees.`);
  }

  console.error(`Sending ${amountSats} sats via LNURL from ${args.walletCurrency} wallet ${wallet.id} (balance: ${formatBalance(wallet)})`);

  const input = {
    walletId: wallet.id,
    lnurl,
    amount: amountSats,
  };

  const data = await graphqlRequest(PAY_LNURL_MUTATION, { input });
  const result = data.lnurlPaymentSend;

  if (result.errors && result.errors.length > 0) {
    const errMsg = result.errors.map(e => `${e.message}${e.code ? ` [${e.code}]` : ''}`).join(', ');
    throw new Error(`Payment failed: ${errMsg}`);
  }

  const output = {
    status: result.status,
    amountSats,
    walletId: wallet.id,
    walletCurrency: args.walletCurrency,
    balanceBefore: wallet.balance,
  };

  if (args.walletCurrency === 'USD') {
    output.balanceBeforeFormatted = `$${(wallet.balance / 100).toFixed(2)}`;
  }

  if (result.status === 'SUCCESS') {
    console.error('Payment successful!');
  } else if (result.status === 'PENDING') {
    console.error('Payment is pending...');
  } else {
    console.error(`Payment status: ${result.status}`);
  }

  console.log(JSON.stringify(output, null, 2));
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});
