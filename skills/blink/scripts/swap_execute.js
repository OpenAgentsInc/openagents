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

  const quoteResult = await estimateSwapQuote({
    direction: parsed.direction,
    amount: parsed.amount,
    unit: parsed.unit,
    ttlSeconds: parsed.ttlSeconds,
    immediateExecution: parsed.immediateExecution,
    apiKey,
    apiUrl,
  });

  if (parsed.dryRun) {
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

  console.log(JSON.stringify(output, null, 2));
}

main().catch((error) => {
  console.error('Error:', error.message);
  process.exit(1);
});
