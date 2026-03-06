#!/usr/bin/env node
/**
 * Blink Wallet - Execute Swap (BTC <-> USD)
 *
 * Usage:
 *   node swap_execute.js <direction> <amount> [--unit sats|cents] [--ttl-seconds N] [--immediate] [--dry-run] [--memo "text"]
 *
 * Direction:
 *   btc-to-usd | usd-to-btc
 *   aliases: sell-btc, buy-usd, sell-usd, buy-btc
 *
 * Examples:
 *   node swap_execute.js btc-to-usd 2000
 *   node swap_execute.js usd-to-btc 500 --unit cents
 *   node swap_execute.js btc-to-usd 2000 --dry-run
 *
 * CAUTION: Without --dry-run this performs a real wallet conversion.
 *
 * Originally authored by @AtlantisPleb (Christopher David) for the OpenAgents
 * project under the Apache License 2.0. See NOTICE file for full attribution.
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const { getApiKey, getApiUrl } = require('./_blink_client');
const {
  parseCommonSwapArgs,
  estimateSwapQuote,
  executeSwap,
  computeBalanceDelta,
} = require('./_swap_common');

async function main() {
  const parsed = parseCommonSwapArgs(process.argv.slice(2));
  if (parsed.error) {
    console.error(`Error: ${parsed.error}`);
    process.exit(1);
  }

  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  console.error(`Estimating swap: ${parsed.direction} ${parsed.amount} ${parsed.unit}...`);

  const quoteResult = await estimateSwapQuote({
    direction: parsed.direction,
    amount: parsed.amount,
    unit: parsed.unit,
    ttlSeconds: parsed.ttlSeconds,
    immediateExecution: parsed.immediateExecution,
    apiKey,
    apiUrl,
  });

  // ── Dry-run: show quote details without executing ──
  if (parsed.dryRun) {
    console.error('[DRY RUN] Would execute swap — no funds will be converted.');
    const output = {
      event: 'swap_execution',
      dryRun: true,
      direction: parsed.direction,
      status: 'DRY_RUN',
      succeeded: false,
      preBalance: quoteResult.preBalance,
      postBalance: quoteResult.preBalance,
      balanceDelta: computeBalanceDelta(quoteResult.preBalance, quoteResult.preBalance),
      quote: quoteResult.quote,
      execution: {
        path: quoteResult.quote.executionPath,
        transactionId: null,
      },
      executedAtEpochSeconds: Math.floor(Date.now() / 1000),
    };
    console.log(JSON.stringify(output, null, 2));
    return;
  }

  // ── Execute the swap ──
  console.error('Executing swap...');

  const executionResult = await executeSwap({
    direction: parsed.direction,
    quote: quoteResult.quote,
    memo: parsed.memo,
    apiKey,
    apiUrl,
  });

  const output = {
    event: 'swap_execution',
    dryRun: false,
    direction: parsed.direction,
    status: executionResult.status,
    succeeded: executionResult.status === 'SUCCESS',
    preBalance: executionResult.preBalance,
    postBalance: executionResult.postBalance,
    balanceDelta: executionResult.balanceDelta,
    quote: quoteResult.quote,
    execution: {
      path: quoteResult.quote.executionPath,
      transactionId: executionResult.transactionId,
      memo: parsed.memo,
    },
    executedAtEpochSeconds: Math.floor(Date.now() / 1000),
  };

  if (executionResult.status === 'SUCCESS') {
    console.error('Swap executed successfully!');
  } else if (executionResult.status === 'PENDING') {
    console.error('Swap is pending...');
  } else {
    console.error(`Swap status: ${executionResult.status}`);
  }

  console.log(JSON.stringify(output, null, 2));
}

if (require.main === module) {
  main().catch((e) => {
    console.error('Error:', e.message);
    process.exit(1);
  });
}

module.exports = { main };
