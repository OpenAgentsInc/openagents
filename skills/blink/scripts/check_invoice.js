#!/usr/bin/env node
/**
 * Blink Wallet - Check Invoice Status
 *
 * Usage: node check_invoice.js <payment_hash>
 *
 * Checks the payment status of a Lightning invoice by its payment hash.
 * Use this after creating an invoice to detect when it has been paid.
 *
 * Arguments:
 *   payment_hash  - Required. The payment hash returned from create_invoice.js.
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
        }
      }
    }
  }
`;

const CHECK_INVOICE_QUERY = `
  query InvoiceByHash($walletId: WalletId!, $paymentHash: PaymentHash!) {
    me {
      defaultAccount {
        walletById(walletId: $walletId) {
          ... on BTCWallet {
            invoiceByPaymentHash(paymentHash: $paymentHash) {
              ... on LnInvoice {
                paymentHash
                paymentRequest
                paymentStatus
                satoshis
                createdAt
                externalId
              }
            }
          }
          ... on UsdWallet {
            invoiceByPaymentHash(paymentHash: $paymentHash) {
              ... on LnInvoice {
                paymentHash
                paymentRequest
                paymentStatus
                satoshis
                createdAt
                externalId
              }
            }
          }
        }
      }
    }
  }
`;

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error('Usage: node check_invoice.js <payment_hash>');
    process.exit(1);
  }

  const paymentHash = args[0].trim();
  if (!/^[a-f0-9]{64}$/.test(paymentHash)) {
    console.error('Warning: payment_hash does not look like a valid 64-char hex string.');
  }

  // Get all wallets and try each one
  const walletData = await graphqlRequest(WALLET_QUERY);
  if (!walletData.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');

  const wallets = walletData.me.defaultAccount.wallets;
  let invoice = null;
  let foundInWallet = null;

  for (const wallet of wallets) {
    try {
      const data = await graphqlRequest(CHECK_INVOICE_QUERY, {
        walletId: wallet.id,
        paymentHash,
      });

      const walletResult = data.me.defaultAccount.walletById;
      const inv = walletResult.invoiceByPaymentHash;
      if (inv && inv.paymentHash) {
        invoice = inv;
        foundInWallet = wallet;
        break;
      }
    } catch {
      // Invoice not found in this wallet, try next
      continue;
    }
  }

  if (!invoice) {
    throw new Error(`Invoice with payment hash ${paymentHash} not found in any wallet.`);
  }

  const output = {
    paymentHash: invoice.paymentHash,
    paymentStatus: invoice.paymentStatus,
    satoshis: invoice.satoshis,
    createdAt: invoice.createdAt,
    paymentRequest: invoice.paymentRequest,
    externalId: invoice.externalId || null,
    walletId: foundInWallet.id,
    walletCurrency: foundInWallet.walletCurrency,
    isPaid: invoice.paymentStatus === 'PAID',
    isExpired: invoice.paymentStatus === 'EXPIRED',
    isPending: invoice.paymentStatus === 'PENDING',
  };

  console.log(JSON.stringify(output, null, 2));
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});