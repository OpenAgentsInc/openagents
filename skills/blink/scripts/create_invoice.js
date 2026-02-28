#!/usr/bin/env node
/**
 * Blink Wallet - Create Lightning Invoice (with auto-subscribe)
 *
 * Usage: node create_invoice.js <amount_sats> [--timeout <seconds>] [--no-subscribe] [memo...]
 *
 * Creates a Lightning invoice (BOLT-11) for the specified amount in satoshis.
 * Automatically resolves the BTC wallet ID from the account.
 *
 * After creating the invoice, automatically opens a WebSocket subscription to
 * watch for payment. Outputs TWO JSON objects to stdout:
 *   1. Immediately: invoice creation result (paymentRequest, paymentHash, etc.)
 *   2. When resolved: payment status (PAID, EXPIRED, TIMEOUT, ERROR)
 *
 * The agent can read the first JSON to share the invoice with the user right away,
 * then wait for the second JSON to know when payment is received.
 *
 * Arguments:
 *   amount_sats    - Required. Amount in satoshis.
 *   --timeout <s>  - Optional. Subscription timeout in seconds (default: 300). Use 0 for no timeout.
 *   --no-subscribe - Optional. Skip WebSocket subscription, just create and exit.
 *   memo...        - Optional. Remaining args joined as memo text.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch and WebSocket)
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const DEFAULT_API_URL = 'https://api.blink.sv/graphql';

// ── Helpers ──────────────────────────────────────────────────────────────────

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

function getWsUrl() {
  const apiUrl = getApiUrl();
  const url = new URL(apiUrl);
  url.protocol = url.protocol === 'http:' ? 'ws:' : 'wss:';
  if (url.hostname.startsWith('api.')) url.hostname = url.hostname.replace(/^api\./, 'ws.');
  return url.toString();
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

// ── Arg parsing ──────────────────────────────────────────────────────────────

function parseArgs(argv) {
  let amountSats = null;
  let timeoutSeconds = 300;
  let noSubscribe = false;
  const memoParts = [];

  let i = 0;
  for (; i < argv.length; i++) {
    const arg = argv[i];

    if (arg === '--timeout') {
      const value = argv[i + 1];
      if (!value) throw new Error('Missing value for --timeout');
      timeoutSeconds = parseInt(value, 10);
      if (isNaN(timeoutSeconds) || timeoutSeconds < 0) throw new Error('--timeout must be a non-negative integer');
      i++;
      continue;
    }

    if (arg === '--no-subscribe') {
      noSubscribe = true;
      continue;
    }

    if (amountSats === null) {
      amountSats = parseInt(arg, 10);
      if (isNaN(amountSats) || amountSats <= 0) throw new Error('amount_sats must be a positive integer');
      continue;
    }

    // Everything else is memo
    memoParts.push(arg);
  }

  return {
    amountSats,
    timeoutSeconds,
    noSubscribe,
    memo: memoParts.length > 0 ? memoParts.join(' ') : undefined,
  };
}

// ── GraphQL queries ──────────────────────────────────────────────────────────

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

const CREATE_INVOICE_MUTATION = `
  mutation LnInvoiceCreate($input: LnInvoiceCreateInput!) {
    lnInvoiceCreate(input: $input) {
      invoice {
        paymentRequest
        paymentHash
        paymentSecret
        satoshis
        paymentStatus
        createdAt
      }
      errors {
        code
        message
        path
      }
    }
  }
`;

// ── Wallet resolution ────────────────────────────────────────────────────────

async function getBtcWalletId() {
  const data = await graphqlRequest(WALLET_QUERY);
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');
  const btcWallet = data.me.defaultAccount.wallets.find(w => w.walletCurrency === 'BTC');
  if (!btcWallet) throw new Error('No BTC wallet found on this account.');
  return btcWallet.id;
}

// ── WebSocket subscription ───────────────────────────────────────────────────

function subscribeToInvoice(paymentRequest, timeoutSeconds) {
  if (typeof WebSocket !== 'function') {
    console.error('Warning: WebSocket not available, skipping auto-subscribe. Run with node --experimental-websocket for auto-subscribe.');
    return;
  }

  const apiKey = getApiKey();
  const wsUrl = getWsUrl();

  let done = false;
  let timeoutId = null;

  const ws = new WebSocket(wsUrl, 'graphql-transport-ws');

  function finish(result, exitCode = 0) {
    if (done) return;
    done = true;
    if (timeoutId) clearTimeout(timeoutId);
    try {
      if (ws.readyState === ws.OPEN) {
        ws.send(JSON.stringify({ id: '1', type: 'complete' }));
      }
    } catch {}
    try {
      ws.close(1000);
    } catch {}
    if (result) {
      console.log(JSON.stringify(result, null, 2));
    }
    process.exit(exitCode);
  }

  if (timeoutSeconds > 0) {
    timeoutId = setTimeout(() => {
      console.error('Subscription timed out.');
      finish({
        event: 'subscription_result',
        paymentRequest,
        status: 'TIMEOUT',
        isPaid: false,
        isExpired: false,
        isPending: true,
      }, 1);
    }, timeoutSeconds * 1000);
  }

  ws.onopen = () => {
    ws.send(JSON.stringify({
      type: 'connection_init',
      payload: { 'X-API-KEY': apiKey },
    }));
  };

  ws.onmessage = (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch (e) {
      console.error('Warning: received non-JSON WebSocket message');
      return;
    }

    if (message.type === 'connection_ack') {
      console.error('Subscribed — waiting for payment...');
      ws.send(JSON.stringify({
        id: '1',
        type: 'subscribe',
        payload: {
          query: `subscription LnInvoicePaymentStatus($input: LnInvoicePaymentStatusInput!) {
  lnInvoicePaymentStatus(input: $input) {
    status
  }
}`,
          variables: { input: { paymentRequest } },
        },
      }));
      return;
    }

    if (message.type === 'ping') {
      ws.send(JSON.stringify({ type: 'pong' }));
      return;
    }

    if (message.type === 'error') {
      console.error('Subscription error:', JSON.stringify(message.payload || message));
      finish({
        event: 'subscription_result',
        paymentRequest,
        status: 'ERROR',
        error: message.payload || message,
      }, 1);
      return;
    }

    if (message.type === 'next') {
      const status = message.payload && message.payload.data && message.payload.data.lnInvoicePaymentStatus
        ? message.payload.data.lnInvoicePaymentStatus.status
        : null;
      if (!status) return;

      console.error(`Invoice status: ${status}`);
      if (status === 'PAID' || status === 'EXPIRED') {
        finish({
          event: 'subscription_result',
          paymentRequest,
          status,
          isPaid: status === 'PAID',
          isExpired: status === 'EXPIRED',
          isPending: false,
        }, 0);
      }
      return;
    }

    if (message.type === 'complete') {
      finish({
        event: 'subscription_result',
        paymentRequest,
        status: 'COMPLETE',
        isPaid: false,
        isExpired: false,
        isPending: true,
      }, 0);
    }
  };

  ws.onerror = () => {
    console.error('WebSocket error during subscription');
    finish({
      event: 'subscription_result',
      paymentRequest,
      status: 'ERROR',
      error: 'WebSocket error',
    }, 1);
  };

  ws.onclose = (event) => {
    if (done) return;
    console.error(`WebSocket closed: code=${event.code} reason=${event.reason || 'unknown'}`);
    finish({
      event: 'subscription_result',
      paymentRequest,
      status: 'CLOSED',
      isPaid: false,
      isExpired: false,
      isPending: true,
    }, 1);
  };
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.amountSats === null) {
    console.error('Usage: node create_invoice.js <amount_sats> [--timeout <seconds>] [--no-subscribe] [memo...]');
    process.exit(1);
  }

  // Auto-resolve BTC wallet ID
  const walletId = await getBtcWalletId();

  const input = { walletId, amount: args.amountSats };
  if (args.memo) input.memo = args.memo;

  const data = await graphqlRequest(CREATE_INVOICE_MUTATION, { input });
  const result = data.lnInvoiceCreate;

  if (result.errors && result.errors.length > 0) {
    throw new Error(`Invoice creation failed: ${result.errors.map(e => e.message).join(', ')}`);
  }

  if (!result.invoice) {
    throw new Error('Invoice creation returned no invoice and no errors.');
  }

  // Phase 1: Output invoice creation result immediately
  const creationResult = {
    event: 'invoice_created',
    paymentRequest: result.invoice.paymentRequest,
    paymentHash: result.invoice.paymentHash,
    satoshis: result.invoice.satoshis,
    status: result.invoice.paymentStatus,
    createdAt: result.invoice.createdAt,
    walletId,
  };

  console.log(JSON.stringify(creationResult, null, 2));

  // Phase 2: Auto-subscribe to payment status (unless opted out)
  if (args.noSubscribe) {
    console.error('Subscription skipped (--no-subscribe).');
    process.exit(0);
  }

  console.error(`Auto-subscribing to invoice payment status (timeout: ${args.timeoutSeconds}s)...`);
  subscribeToInvoice(result.invoice.paymentRequest, args.timeoutSeconds);
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});