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

const { getApiKey, getApiUrl, graphqlRequest, getAllWallets } = require('./_blink_client');

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

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  // Get all wallets and try each one
  const wallets = await getAllWallets({ apiKey, apiUrl });
  let invoice = null;
  let foundInWallet = null;

  for (const wallet of wallets) {
    const data = await graphqlRequest({
      query: CHECK_INVOICE_QUERY,
      variables: { walletId: wallet.id, paymentHash },
      apiKey,
      apiUrl,
    });

    const walletResult = data.me.defaultAccount.walletById;
    const inv = walletResult.invoiceByPaymentHash;
    if (inv && inv.paymentHash) {
      invoice = inv;
      foundInWallet = wallet;
      break;
    }
    // Invoice not found in this wallet — continue to next
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

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
