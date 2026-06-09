#!/usr/bin/env node
/**
 * Blink Wallet - Swap Quote (BTC <-> USD)
 *
 * Usage:
 *   node swap_quote.js <direction> <amount> [--unit sats|cents] [--ttl-seconds N] [--immediate]
 *
 * Direction:
 *   btc-to-usd | usd-to-btc
 *   aliases: sell-btc, buy-usd, sell-usd, buy-btc
 *
 * Examples:
 *   node swap_quote.js btc-to-usd 1000
 *   node swap_quote.js usd-to-btc 500 --unit cents
 *   node swap_quote.js btc-to-usd 1000 --immediate --ttl-seconds 45
 *
 * Originally authored by @AtlantisPleb (Christopher David) for the OpenAgents
 * project under the Apache License 2.0. See NOTICE file for full attribution.
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const { getApiKey, getApiUrl } = require('./_blink_client');
const { parseCommonSwapArgs, estimateSwapQuote } = require('./_swap_common');

async function main() {
  const parsed = parseCommonSwapArgs(process.argv.slice(2));
  if (parsed.error) {
    console.error(`Error: ${parsed.error}`);
    process.exit(1);
  }

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  console.error(`Estimating swap quote: ${parsed.direction} ${parsed.amount} ${parsed.unit}...`);

  const quoteResult = await estimateSwapQuote({
    direction: parsed.direction,
    amount: parsed.amount,
    unit: parsed.unit,
    ttlSeconds: parsed.ttlSeconds,
    immediateExecution: parsed.immediateExecution,
    apiKey,
    apiUrl,
  });

  const output = {
    event: 'swap_quote',
    dryRun: true,
    direction: parsed.direction,
    preBalance: quoteResult.preBalance,
    quote: quoteResult.quote,
    generatedAtEpochSeconds: Math.floor(Date.now() / 1000),
  };

  console.error('Quote generated successfully.');
  console.log(JSON.stringify(output, null, 2));
}

if (require.main === module) {
  main().catch((e) => {
    console.error('Error:', e.message);
    process.exit(1);
  });
}

module.exports = { main };
