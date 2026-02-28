#!/usr/bin/env node
/**
 * Blink Wallet - Pay Lightning Invoice
 *
 * Usage: node pay_invoice.js <bolt11_invoice> [--wallet BTC|USD]
 *
 * Pays a BOLT-11 Lightning invoice from the BTC or USD wallet.
 * Automatically resolves the wallet ID from the account.
 *
 * Arguments:
 *   bolt11_invoice  - Required. The BOLT-11 payment request string (lnbc...).
 *   --wallet        - Optional. Wallet to pay from: BTC (default) or USD.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 *
 * CAUTION: This sends real bitcoin. The API key must have Write scope.
 */

const {
  getApiKey,
  getApiUrl,
  graphqlRequest,
  getWallet,
  formatBalance,
  parseWalletArg,
  normalizeInvoice,
  warnIfNotBolt11,
  MUTATION_TIMEOUT_MS,
} = require('./_blink_client');

const PAY_INVOICE_MUTATION = `
  mutation LnInvoicePaymentSend($input: LnInvoicePaymentInput!) {
    lnInvoicePaymentSend(input: $input) {
      status
      errors {
        code
        message
        path
      }
    }
  }
`;

async function main() {
  const { walletCurrency, remaining } = parseWalletArg(process.argv.slice(2));
  const rawInvoice = remaining[0];

  if (!rawInvoice) {
    console.error('Usage: node pay_invoice.js <bolt11_invoice> [--wallet BTC|USD]');
    process.exit(1);
  }

  const paymentRequest = normalizeInvoice(rawInvoice);
  warnIfNotBolt11(paymentRequest);

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  // Resolve wallet (BTC or USD)
  const wallet = await getWallet({ apiKey, apiUrl, currency: walletCurrency });

  console.error(`Using ${walletCurrency} wallet ${wallet.id} (balance: ${formatBalance(wallet)})`);

  const input = {
    walletId: wallet.id,
    paymentRequest,
  };

  const data = await graphqlRequest({
    query: PAY_INVOICE_MUTATION,
    variables: { input },
    apiKey,
    apiUrl,
    timeoutMs: MUTATION_TIMEOUT_MS,
  });
  const result = data.lnInvoicePaymentSend;

  if (result.errors && result.errors.length > 0) {
    const errMsg = result.errors.map((e) => `${e.message}${e.code ? ` [${e.code}]` : ''}`).join(', ');
    throw new Error(`Payment failed: ${errMsg}`);
  }

  const output = {
    status: result.status,
    walletId: wallet.id,
    walletCurrency,
    balanceBefore: wallet.balance,
  };

  if (walletCurrency === 'USD') {
    output.balanceBeforeFormatted = `$${(wallet.balance / 100).toFixed(2)}`;
  }

  if (result.status === 'SUCCESS') {
    console.error('Payment successful!');
  } else if (result.status === 'PENDING') {
    console.error('Payment is pending...');
  } else if (result.status === 'ALREADY_PAID') {
    console.error('Invoice was already paid.');
  } else {
    console.error(`Payment status: ${result.status}`);
  }

  console.log(JSON.stringify(output, null, 2));
}

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
