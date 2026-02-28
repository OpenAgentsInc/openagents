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

const {
  getApiKey,
  getApiUrl,
  graphqlRequest,
  getWallet,
  formatBalance,
  parseWalletArg,
  MUTATION_TIMEOUT_MS,
} = require('./_blink_client');

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

async function main() {
  const { walletCurrency, remaining } = parseWalletArg(process.argv.slice(2));
  const lnurl = remaining[0] ? remaining[0].trim() : null;
  const amountSats = remaining[1] ? parseInt(remaining[1], 10) : null;

  if (!lnurl || amountSats === null) {
    console.error('Usage: node pay_lnurl.js <lnurl> <amount_sats> [--wallet BTC|USD]');
    process.exit(1);
  }

  if (!lnurl.toLowerCase().startsWith('lnurl')) {
    console.error('Warning: input does not start with "lnurl" — may not be a valid LNURL string.');
    console.error('For Lightning Addresses (user@domain), use pay_lnaddress.js instead.');
  }

  if (isNaN(amountSats) || amountSats <= 0) {
    console.error('Error: amount_sats must be a positive integer');
    process.exit(1);
  }

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  // Resolve wallet (BTC or USD)
  const wallet = await getWallet({ apiKey, apiUrl, currency: walletCurrency });

  // Balance warning (BTC balance is in sats, directly comparable; USD balance
  // is in cents, not directly comparable to sats — skip for USD)
  if (walletCurrency === 'BTC' && wallet.balance < amountSats) {
    console.error(
      `Warning: wallet balance (${wallet.balance} sats) may be insufficient for ${amountSats} sats + fees.`,
    );
  }

  console.error(
    `Sending ${amountSats} sats via LNURL from ${walletCurrency} wallet ${wallet.id} (balance: ${formatBalance(wallet)})`,
  );

  const input = {
    walletId: wallet.id,
    lnurl,
    amount: amountSats,
  };

  const data = await graphqlRequest({
    query: PAY_LNURL_MUTATION,
    variables: { input },
    apiKey,
    apiUrl,
    timeoutMs: MUTATION_TIMEOUT_MS,
  });
  const result = data.lnurlPaymentSend;

  if (result.errors && result.errors.length > 0) {
    const errMsg = result.errors.map((e) => `${e.message}${e.code ? ` [${e.code}]` : ''}`).join(', ');
    throw new Error(`Payment failed: ${errMsg}`);
  }

  const output = {
    status: result.status,
    amountSats,
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
  } else {
    console.error(`Payment status: ${result.status}`);
  }

  console.log(JSON.stringify(output, null, 2));
}

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
