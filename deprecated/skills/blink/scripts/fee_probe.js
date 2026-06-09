#!/usr/bin/env node
/**
 * Blink Wallet - Fee Probe (Estimate Payment Fee)
 *
 * Usage: node fee_probe.js <bolt11_invoice> [--wallet BTC|USD]
 *
 * Estimates the fee for paying a Lightning invoice without actually sending.
 * Use this to check costs before committing to a payment.
 *
 * When --wallet USD is specified, uses the lnUsdInvoiceFeeProbe mutation
 * to estimate fees from the USD wallet's perspective.
 *
 * Arguments:
 *   bolt11_invoice  - Required. The BOLT-11 payment request string (lnbc...).
 *   --wallet        - Optional. Wallet to probe from: BTC (default) or USD.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const {
  getApiKey,
  getApiUrl,
  graphqlRequest,
  getWallet,
  parseWalletArg,
  normalizeInvoice,
  warnIfNotBolt11,
  MUTATION_TIMEOUT_MS,
} = require('./_blink_client');

const FEE_PROBE_BTC_MUTATION = `
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

const FEE_PROBE_USD_MUTATION = `
  mutation LnUsdInvoiceFeeProbe($input: LnUsdInvoiceFeeProbeInput!) {
    lnUsdInvoiceFeeProbe(input: $input) {
      amount
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
    console.error('Usage: node fee_probe.js <bolt11_invoice> [--wallet BTC|USD]');
    process.exit(1);
  }

  const paymentRequest = normalizeInvoice(rawInvoice);
  warnIfNotBolt11(paymentRequest);

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  const wallet = await getWallet({ apiKey, apiUrl, currency: walletCurrency });

  const input = {
    walletId: wallet.id,
    paymentRequest,
  };

  // Use the appropriate fee probe mutation based on wallet currency
  const mutation = walletCurrency === 'USD' ? FEE_PROBE_USD_MUTATION : FEE_PROBE_BTC_MUTATION;
  const mutationKey = walletCurrency === 'USD' ? 'lnUsdInvoiceFeeProbe' : 'lnInvoiceFeeProbe';

  const data = await graphqlRequest({
    query: mutation,
    variables: { input },
    apiKey,
    apiUrl,
    timeoutMs: MUTATION_TIMEOUT_MS,
  });
  const result = data[mutationKey];

  if (result.errors && result.errors.length > 0) {
    const errMsg = result.errors.map((e) => `${e.message}${e.code ? ` [${e.code}]` : ''}`).join(', ');
    throw new Error(`Fee probe failed: ${errMsg}`);
  }

  const output = {
    estimatedFeeSats: result.amount,
    walletId: wallet.id,
    walletCurrency,
    walletBalance: wallet.balance,
  };

  if (walletCurrency === 'USD') {
    output.walletBalanceFormatted = `$${(wallet.balance / 100).toFixed(2)}`;
  }

  if (result.amount === 0) {
    console.error('Fee estimate: 0 sats (internal transfer or direct channel)');
  } else {
    console.error(`Fee estimate: ${result.amount} sats`);
  }

  console.log(JSON.stringify(output, null, 2));
}

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
