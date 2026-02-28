/**
 * Blink Claw Skill — Shared Client Module
 *
 * Centralises API key resolution, GraphQL requests, WebSocket helpers,
 * invoice normalisation, wallet resolution, and common arg-parsing logic
 * used by every script in blink/scripts/.
 *
 * Zero external dependencies — Node.js 18+ built-ins only.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

// ── Constants ────────────────────────────────────────────────────────────────

const DEFAULT_API_URL = 'https://api.blink.sv/graphql';
const DEFAULT_TIMEOUT_MS = 15_000;
const MUTATION_TIMEOUT_MS = 30_000;

// ── Config helpers ───────────────────────────────────────────────────────────

/**
 * Resolve the Blink API key from the environment or ~/.profile.
 *
 * @param {object}  [opts]
 * @param {boolean} [opts.required=true]  Throw if the key is not found.
 * @returns {string|null}
 */
function getApiKey({ required = true } = {}) {
  let key = process.env.BLINK_API_KEY;
  if (!key) {
    try {
      const profile = fs.readFileSync(path.join(os.homedir(), '.profile'), 'utf8');
      const match = profile.match(/BLINK_API_KEY=["']?([a-zA-Z0-9_]+)["']?/);
      if (match) key = match[1];
    } catch {
      // ~/.profile not readable — fall through
    }
  }
  if (!key && required) {
    throw new Error('BLINK_API_KEY not found. Set it in environment or ~/.profile');
  }
  return key || null;
}

/**
 * Resolve the Blink GraphQL API URL.
 * @returns {string}
 */
function getApiUrl() {
  return process.env.BLINK_API_URL || DEFAULT_API_URL;
}

/**
 * Resolve the Blink WebSocket URL.
 * Prefers BLINK_WS_URL env override; otherwise derives from the API URL.
 * @returns {string}
 */
function getWsUrl() {
  if (process.env.BLINK_WS_URL) return process.env.BLINK_WS_URL;
  const apiUrl = getApiUrl();
  const url = new URL(apiUrl);
  url.protocol = url.protocol === 'http:' ? 'ws:' : 'wss:';
  if (url.hostname.startsWith('api.')) {
    url.hostname = url.hostname.replace(/^api\./, 'ws.');
  }
  return url.toString();
}

// ── HTTP / GraphQL ───────────────────────────────────────────────────────────

/**
 * Execute a GraphQL request against the Blink API.
 *
 * @param {object}  opts
 * @param {string}  opts.query          GraphQL query or mutation string.
 * @param {object}  [opts.variables={}] GraphQL variables.
 * @param {string|null} [opts.apiKey]   API key (null ⇒ unauthenticated).
 * @param {string}  [opts.apiUrl]       API endpoint URL.
 * @param {number}  [opts.timeoutMs]    Request timeout in ms (default 15 000).
 * @returns {object} The `data` property of the GraphQL response.
 */
async function graphqlRequest({
  query,
  variables = {},
  apiKey = null,
  apiUrl = DEFAULT_API_URL,
  timeoutMs = DEFAULT_TIMEOUT_MS,
} = {}) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const headers = { 'Content-Type': 'application/json' };
    if (apiKey) headers['X-API-KEY'] = apiKey;

    const res = await fetch(apiUrl, {
      method: 'POST',
      headers,
      body: JSON.stringify({ query, variables }),
      signal: controller.signal,
    });

    if (!res.ok) {
      throw new Error(`HTTP ${res.status}: ${await res.text()}`);
    }

    const json = await res.json();
    if (json.errors && json.errors.length > 0) {
      throw new Error(`GraphQL error: ${json.errors.map((e) => e.message).join(', ')}`);
    }
    return json.data;
  } finally {
    clearTimeout(timer);
  }
}

// ── Invoice helpers ──────────────────────────────────────────────────────────

/**
 * Trim whitespace and strip a leading `lightning:` URI prefix (case-insensitive).
 * @param {string} input  Raw invoice / payment-request string.
 * @returns {string}
 */
function normalizeInvoice(input) {
  const trimmed = input.trim();
  if (trimmed.toLowerCase().startsWith('lightning:')) {
    return trimmed.slice('lightning:'.length);
  }
  return trimmed;
}

/**
 * Emit a warning to stderr if the invoice doesn't look like a valid BOLT-11
 * payment request. Checks are case-insensitive.
 * @param {string} invoice  The (already-normalised) invoice string.
 */
function warnIfNotBolt11(invoice) {
  const lower = invoice.toLowerCase();
  if (!lower.startsWith('lnbc') && !lower.startsWith('lntbs') && !lower.startsWith('lntb')) {
    console.error('Warning: invoice does not start with lnbc/lntbs/lntb \u2014 may not be a valid BOLT-11 invoice.');
  }
}

// ── Wallet helpers ───────────────────────────────────────────────────────────

const WALLET_QUERY = `
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

/**
 * Resolve a wallet by currency.
 *
 * @param {object}  opts
 * @param {string}  opts.apiKey
 * @param {string}  opts.apiUrl
 * @param {string}  opts.currency       "BTC" or "USD".
 * @param {number}  [opts.timeoutMs]
 * @returns {{ id: string, walletCurrency: string, balance: number, pendingIncomingBalance: number }}
 */
async function getWallet({ apiKey, apiUrl, currency, timeoutMs }) {
  const data = await graphqlRequest({ query: WALLET_QUERY, apiKey, apiUrl, timeoutMs });
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');
  const wallet = data.me.defaultAccount.wallets.find((w) => w.walletCurrency === currency);
  if (!wallet) throw new Error(`No ${currency} wallet found on this account.`);
  return wallet;
}

/**
 * Resolve all wallets on the account.
 *
 * @param {object}  opts
 * @param {string}  opts.apiKey
 * @param {string}  opts.apiUrl
 * @param {number}  [opts.timeoutMs]
 * @returns {Array}
 */
async function getAllWallets({ apiKey, apiUrl, timeoutMs }) {
  const data = await graphqlRequest({ query: WALLET_QUERY, apiKey, apiUrl, timeoutMs });
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');
  return data.me.defaultAccount.wallets;
}

// ── Currency conversion ──────────────────────────────────────────────────────

const CONVERSION_QUERY = `
  query CurrencyConversion($amount: Float!, $currency: DisplayCurrency!) {
    currencyConversionEstimation(amount: $amount, currency: $currency) {
      btcSatAmount
      usdCentAmount
    }
  }
`;

/**
 * Estimate the USD value of a satoshi amount.
 * Non-fatal: returns null on failure so callers can treat it as best-effort.
 *
 * @param {object}  opts
 * @param {number}  opts.sats
 * @param {string}  opts.apiKey
 * @param {string}  opts.apiUrl
 * @param {number}  [opts.timeoutMs]
 * @returns {number|null}  USD value rounded to 2 decimals, or null.
 */
async function estimateSatsToUsd({ sats, apiKey, apiUrl, timeoutMs }) {
  if (sats === 0) return 0;
  try {
    const data = await graphqlRequest({
      query: CONVERSION_QUERY,
      variables: { amount: 1.0, currency: 'USD' },
      apiKey,
      apiUrl,
      timeoutMs,
    });
    const est = data.currencyConversionEstimation;
    if (!est || !est.btcSatAmount || est.btcSatAmount === 0) return null;
    const usdPerSat = 1.0 / est.btcSatAmount;
    return Math.round(sats * usdPerSat * 100) / 100;
  } catch {
    return null; // non-fatal — USD estimate is best-effort
  }
}

// ── Formatting ───────────────────────────────────────────────────────────────

/**
 * Human-readable wallet balance string.
 * @param {{ walletCurrency: string, balance: number }} wallet
 * @returns {string}
 */
function formatBalance(wallet) {
  if (wallet.walletCurrency === 'USD') {
    return `$${(wallet.balance / 100).toFixed(2)} (${wallet.balance} cents)`;
  }
  return `${wallet.balance} sats`;
}

/**
 * Format a cent amount as USD.
 * @param {number} cents
 * @returns {string}
 */
function formatUsdCents(cents) {
  return `$${(cents / 100).toFixed(2)}`;
}

// ── Arg parsing helpers ──────────────────────────────────────────────────────

/**
 * Parse a --wallet BTC|USD flag from an argv array.
 * Returns the chosen currency and the remaining args (without --wallet and its value).
 *
 * @param {string[]} argv  Typically `process.argv.slice(2)`.
 * @returns {{ walletCurrency: string, remaining: string[] }}
 */
function parseWalletArg(argv) {
  let walletCurrency = 'BTC';
  const remaining = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--wallet' && i + 1 < argv.length) {
      const val = argv[i + 1].toUpperCase();
      if (val !== 'BTC' && val !== 'USD') {
        console.error('Error: --wallet must be BTC or USD');
        process.exit(1);
      }
      walletCurrency = val;
      i++; // skip value
    } else {
      remaining.push(argv[i]);
    }
  }
  return { walletCurrency, remaining };
}

// ── WebSocket helpers ────────────────────────────────────────────────────────

/**
 * Ensure WebSocket is available (Node 22+ built-in, or --experimental-websocket).
 * @returns {typeof WebSocket}
 */
function requireWebSocket() {
  if (typeof WebSocket !== 'function') {
    throw new Error('WebSocket is not available. Run with: node --experimental-websocket <script> ...');
  }
  return WebSocket;
}

/**
 * Open a WebSocket subscription to watch an invoice's payment status.
 * Calls `onResult(resultObj, exitCode)` when the subscription resolves.
 *
 * @param {object}  opts
 * @param {string}  opts.paymentRequest   BOLT-11 invoice string.
 * @param {string}  opts.apiKey           Blink API key.
 * @param {string}  opts.wsUrl            WebSocket endpoint URL.
 * @param {number}  opts.timeoutSeconds   Timeout in seconds (0 = no timeout).
 * @param {(result: object, exitCode: number) => void} opts.onResult  Callback.
 */
function subscribeToInvoice({ paymentRequest, apiKey, wsUrl, timeoutSeconds, onResult }) {
  const WebSocketImpl = requireWebSocket();

  let done = false;
  let timeoutId = null;

  const ws = new WebSocketImpl(wsUrl, 'graphql-transport-ws');

  function finish(result, exitCode = 0) {
    if (done) return;
    done = true;
    if (timeoutId) clearTimeout(timeoutId);
    try {
      if (ws.readyState === ws.OPEN) {
        ws.send(JSON.stringify({ id: '1', type: 'complete' }));
      }
    } catch {
      // best-effort cleanup
    }
    try {
      ws.close(1000);
    } catch {
      // best-effort cleanup
    }
    onResult(result, exitCode);
  }

  if (timeoutSeconds > 0) {
    timeoutId = setTimeout(() => {
      console.error('Subscription timed out.');
      finish(
        {
          event: 'subscription_result',
          paymentRequest,
          status: 'TIMEOUT',
          isPaid: false,
          isExpired: false,
          isPending: true,
        },
        1,
      );
    }, timeoutSeconds * 1000);
  }

  ws.onopen = () => {
    ws.send(
      JSON.stringify({
        type: 'connection_init',
        payload: { 'X-API-KEY': apiKey },
      }),
    );
  };

  ws.onmessage = (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch {
      console.error('Warning: received non-JSON WebSocket message');
      return;
    }

    if (message.type === 'connection_ack') {
      console.error('Subscribed \u2014 waiting for payment...');
      ws.send(
        JSON.stringify({
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
        }),
      );
      return;
    }

    if (message.type === 'ping') {
      ws.send(JSON.stringify({ type: 'pong' }));
      return;
    }

    if (message.type === 'error') {
      console.error('Subscription error:', JSON.stringify(message.payload || message));
      finish(
        {
          event: 'subscription_result',
          paymentRequest,
          status: 'ERROR',
          error: message.payload || message,
        },
        1,
      );
      return;
    }

    if (message.type === 'next') {
      const status =
        message.payload && message.payload.data && message.payload.data.lnInvoicePaymentStatus
          ? message.payload.data.lnInvoicePaymentStatus.status
          : null;
      if (!status) return;

      console.error(`Invoice status: ${status}`);
      if (status === 'PAID' || status === 'EXPIRED') {
        finish(
          {
            event: 'subscription_result',
            paymentRequest,
            status,
            isPaid: status === 'PAID',
            isExpired: status === 'EXPIRED',
            isPending: false,
          },
          0,
        );
      }
      return;
    }

    if (message.type === 'complete') {
      finish(
        {
          event: 'subscription_result',
          paymentRequest,
          status: 'COMPLETE',
          isPaid: false,
          isExpired: false,
          isPending: true,
        },
        0,
      );
    }
  };

  ws.onerror = () => {
    console.error('WebSocket error during subscription');
    finish(
      {
        event: 'subscription_result',
        paymentRequest,
        status: 'ERROR',
        error: 'WebSocket error',
      },
      1,
    );
  };

  ws.onclose = (event) => {
    if (done) return;
    console.error(`WebSocket closed: code=${event.code} reason=${event.reason || 'unknown'}`);
    finish(
      {
        event: 'subscription_result',
        paymentRequest,
        status: 'CLOSED',
        isPaid: false,
        isExpired: false,
        isPending: true,
      },
      1,
    );
  };
}

// ── Exports ──────────────────────────────────────────────────────────────────

module.exports = {
  // Constants
  DEFAULT_API_URL,
  DEFAULT_TIMEOUT_MS,
  MUTATION_TIMEOUT_MS,

  // Config
  getApiKey,
  getApiUrl,
  getWsUrl,

  // HTTP
  graphqlRequest,

  // Invoice
  normalizeInvoice,
  warnIfNotBolt11,

  // Wallet
  WALLET_QUERY,
  getWallet,
  getAllWallets,

  // Currency
  CONVERSION_QUERY,
  estimateSatsToUsd,

  // Formatting
  formatBalance,
  formatUsdCents,

  // Arg parsing
  parseWalletArg,

  // WebSocket
  requireWebSocket,
  subscribeToInvoice,
};
