#!/usr/bin/env node
/**
 * Blink Wallet - Create USD-Denominated Lightning Invoice (with auto-subscribe)
 *
 * Usage: node create_invoice_usd.js <amount_cents> [--timeout <seconds>] [--no-subscribe] [memo...]
 *
 * Creates a Lightning invoice denominated in USD cents. The sender pays in
 * BTC/Lightning, but the amount received is locked to a USD value at the
 * exchange rate at invoice creation time. Credited to the USD wallet.
 *
 * NOTE: USD invoices have a short expiry (~5 minutes) because they lock
 * an exchange rate. Use BTC invoices (create_invoice.js) if you need
 * longer-lived invoices.
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
 *   amount_cents   - Required. Amount in USD cents (e.g. 100 = $1.00).
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

const {
  getApiKey,
  getApiUrl,
  getWsUrl,
  graphqlRequest,
  getWallet,
  subscribeToInvoice,
  MUTATION_TIMEOUT_MS,
} = require('./_blink_client');

// ── Arg parsing ──────────────────────────────────────────────────────────────

function parseArgs(argv) {
  let amountCents = null;
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

    if (amountCents === null) {
      amountCents = parseInt(arg, 10);
      if (isNaN(amountCents) || amountCents <= 0) throw new Error('amount_cents must be a positive integer');
      continue;
    }

    // Everything else is memo
    memoParts.push(arg);
  }

  return {
    amountCents,
    timeoutSeconds,
    noSubscribe,
    memo: memoParts.length > 0 ? memoParts.join(' ') : undefined,
  };
}

// ── GraphQL queries ──────────────────────────────────────────────────────────

const CREATE_USD_INVOICE_MUTATION = `
  mutation LnUsdInvoiceCreate($input: LnUsdInvoiceCreateInput!) {
    lnUsdInvoiceCreate(input: $input) {
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

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.amountCents === null) {
    console.error('Usage: node create_invoice_usd.js <amount_cents> [--timeout <seconds>] [--no-subscribe] [memo...]');
    console.error('  amount_cents: amount in USD cents (e.g. 100 = $1.00)');
    process.exit(1);
  }

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  // Auto-resolve USD wallet ID
  const wallet = await getWallet({ apiKey, apiUrl, currency: 'USD' });
  const walletId = wallet.id;

  const input = { walletId, amount: args.amountCents };
  if (args.memo) input.memo = args.memo;

  const data = await graphqlRequest({
    query: CREATE_USD_INVOICE_MUTATION,
    variables: { input },
    apiKey,
    apiUrl,
    timeoutMs: MUTATION_TIMEOUT_MS,
  });
  const result = data.lnUsdInvoiceCreate;

  if (result.errors && result.errors.length > 0) {
    throw new Error(`Invoice creation failed: ${result.errors.map((e) => e.message).join(', ')}`);
  }

  if (!result.invoice) {
    throw new Error('Invoice creation returned no invoice and no errors.');
  }

  const usdFormatted = `$${(args.amountCents / 100).toFixed(2)}`;
  console.error(`Created USD invoice for ${usdFormatted} (${result.invoice.satoshis} sats at current rate)`);
  console.error('Note: USD invoices expire in ~5 minutes due to exchange rate lock.');

  // Phase 1: Output invoice creation result immediately
  const creationResult = {
    event: 'invoice_created',
    paymentRequest: result.invoice.paymentRequest,
    paymentHash: result.invoice.paymentHash,
    satoshis: result.invoice.satoshis,
    amountCents: args.amountCents,
    amountUsd: usdFormatted,
    status: result.invoice.paymentStatus,
    createdAt: result.invoice.createdAt,
    walletId,
    walletCurrency: 'USD',
  };

  console.log(JSON.stringify(creationResult, null, 2));

  // Phase 2: Auto-subscribe to payment status (unless opted out)
  if (args.noSubscribe) {
    console.error('Subscription skipped (--no-subscribe).');
    process.exit(0);
  }

  // Graceful fallback when WebSocket is not available
  if (typeof WebSocket !== 'function') {
    console.error(
      'Warning: WebSocket not available, skipping auto-subscribe. Run with node --experimental-websocket for auto-subscribe.',
    );
    return;
  }

  console.error(`Auto-subscribing to invoice payment status (timeout: ${args.timeoutSeconds}s)...`);
  subscribeToInvoice({
    paymentRequest: result.invoice.paymentRequest,
    apiKey,
    wsUrl: getWsUrl(),
    timeoutSeconds: args.timeoutSeconds,
    onResult(resultObj, exitCode) {
      console.log(JSON.stringify(resultObj, null, 2));
      process.exit(exitCode);
    },
  });
}

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
