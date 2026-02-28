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

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.amountSats === null) {
    console.error('Usage: node create_invoice.js <amount_sats> [--timeout <seconds>] [--no-subscribe] [memo...]');
    process.exit(1);
  }

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  // Auto-resolve BTC wallet ID
  const wallet = await getWallet({ apiKey, apiUrl, currency: 'BTC' });
  const walletId = wallet.id;

  const input = { walletId, amount: args.amountSats };
  if (args.memo) input.memo = args.memo;

  const data = await graphqlRequest({
    query: CREATE_INVOICE_MUTATION,
    variables: { input },
    apiKey,
    apiUrl,
    timeoutMs: MUTATION_TIMEOUT_MS,
  });
  const result = data.lnInvoiceCreate;

  if (result.errors && result.errors.length > 0) {
    throw new Error(`Invoice creation failed: ${result.errors.map((e) => e.message).join(', ')}`);
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
