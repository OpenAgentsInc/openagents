#!/usr/bin/env node
/**
 * Blink Wallet - Check Balances
 *
 * Usage: node balance.js
 *
 * Queries the Blink API for all wallet balances (BTC and USD).
 * Outputs JSON with wallet IDs, currencies, and balances.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const { getApiKey, getApiUrl, getAllWallets, estimateSatsToUsd } = require('./_blink_client');

async function main() {
  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  const wallets = await getAllWallets({ apiKey, apiUrl });

  const result = {
    wallets: wallets.map((w) => ({
      id: w.id,
      currency: w.walletCurrency,
      balance: w.balance,
      pendingIncoming: w.pendingIncomingBalance,
      unit: w.walletCurrency === 'BTC' ? 'sats' : 'cents',
    })),
  };

  // Add convenience top-level fields
  const btc = wallets.find((w) => w.walletCurrency === 'BTC');
  const usd = wallets.find((w) => w.walletCurrency === 'USD');
  if (btc) {
    result.btcWalletId = btc.id;
    result.btcBalance = btc.balance;
    result.btcBalanceSats = btc.balance;

    // Pre-compute USD estimate so the agent doesn't have to do its own math
    const usdEstimate = await estimateSatsToUsd({ sats: btc.balance, apiKey, apiUrl });
    if (usdEstimate !== null) {
      result.btcBalanceUsd = usdEstimate;
      result.btcBalanceUsdFormatted = `$${usdEstimate.toFixed(2)}`;
    }
  }
  if (usd) {
    result.usdWalletId = usd.id;
    result.usdBalance = usd.balance;
    result.usdBalanceCents = usd.balance;
    result.usdBalanceFormatted = `$${(usd.balance / 100).toFixed(2)}`;
  }

  console.log(JSON.stringify(result, null, 2));
}

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
