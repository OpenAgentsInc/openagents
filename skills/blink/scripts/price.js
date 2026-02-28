#!/usr/bin/env node
/**
 * Blink Wallet - BTC/USD Price & Exchange Rates
 *
 * Usage:
 *   node price.js                         — Current BTC/USD price
 *   node price.js <amount_sats>           — Convert sats to USD
 *   node price.js --usd <amount_usd>      — Convert USD to sats
 *   node price.js --history <range>       — Historical BTC prices
 *   node price.js --currencies            — List supported currencies
 *   node price.js --raw                   — Current price with raw realtimePrice data
 *
 * History ranges: ONE_DAY, ONE_WEEK, ONE_MONTH, ONE_YEAR, FIVE_YEARS
 *
 * All price queries are PUBLIC (no API key required). If BLINK_API_KEY is
 * set, it will be included but is not necessary for price lookups.
 *
 * Environment:
 *   BLINK_API_KEY  - Optional for price queries.
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const { getApiKey, getApiUrl, graphqlRequest, CONVERSION_QUERY } = require('./_blink_client');

// ── Queries ─────────────────────────────────────────────────

// Public: realtime price with raw base/offset (supplementary data)
const REALTIME_PRICE_QUERY = `
  query RealtimePrice($currency: DisplayCurrency!) {
    realtimePrice(currency: $currency) {
      id
      timestamp
      btcSatPrice {
        base
        offset
      }
      usdCentPrice {
        base
        offset
      }
      denominatorCurrency
    }
  }
`;

// Public: historical BTC prices
const BTC_PRICE_LIST_QUERY = `
  query BtcPriceList($range: PriceGraphRange!) {
    btcPriceList(range: $range) {
      timestamp
      price {
        base
        offset
        formattedAmount
      }
    }
  }
`;

// Public: list all supported display currencies
const CURRENCY_LIST_QUERY = `
  query CurrencyList {
    currencyList {
      id
      symbol
      name
      flag
      fractionDigits
    }
  }
`;

// ── Subcommands ─────────────────────────────────────────────

async function cmdCurrentPrice(apiKey, apiUrl, includeRaw) {
  // Use currencyConversionEstimation to derive BTC/USD price from a $1 reference
  const data = await graphqlRequest({
    query: CONVERSION_QUERY,
    variables: { amount: 1.0, currency: 'USD' },
    apiKey,
    apiUrl,
  });
  const est = data.currencyConversionEstimation;
  // est.btcSatAmount = how many sats $1.00 buys
  const satsPerDollar = est.btcSatAmount;
  const btcPriceUsd = 100_000_000 / satsPerDollar;
  const btcPriceRounded = Math.round(btcPriceUsd * 100) / 100;

  // Optionally fetch raw realtimePrice for supplementary data
  let raw = null;
  if (includeRaw) {
    try {
      const rtData = await graphqlRequest({
        query: REALTIME_PRICE_QUERY,
        variables: { currency: 'USD' },
        apiKey,
        apiUrl,
      });
      raw = rtData.realtimePrice;
    } catch {
      // non-fatal
    }
  }

  const output = {
    btcPriceUsd: btcPriceRounded,
    satsPerDollar,
    timestamp: raw ? raw.timestamp : new Date().toISOString(),
  };

  if (raw) {
    output.raw = {
      btcSatPrice: { base: raw.btcSatPrice.base, offset: raw.btcSatPrice.offset },
      usdCentPrice: { base: raw.usdCentPrice.base, offset: raw.usdCentPrice.offset },
      denominatorCurrency: raw.denominatorCurrency,
    };
  }

  console.error(`BTC price: $${btcPriceRounded.toLocaleString()} USD (${satsPerDollar} sats/$1)`);
  console.log(JSON.stringify(output, null, 2));
}

async function cmdSatsToUsd(amountSats, apiKey, apiUrl) {
  // Ask API: "how many sats is $1 worth?" then derive the rate
  const data = await graphqlRequest({
    query: CONVERSION_QUERY,
    variables: { amount: 1.0, currency: 'USD' },
    apiKey,
    apiUrl,
  });
  const est = data.currencyConversionEstimation;
  const satsPerDollar = est.btcSatAmount;
  const usdValue = amountSats / satsPerDollar;
  const usdRounded = Math.round(usdValue * 100) / 100;
  const btcPriceUsd = Math.round((100_000_000 / satsPerDollar) * 100) / 100;

  const output = {
    btcPriceUsd,
    satsPerDollar,
    conversion: {
      sats: amountSats,
      usd: usdRounded,
      usdFormatted: `$${usdRounded.toFixed(2)}`,
    },
  };

  console.error(
    `${amountSats.toLocaleString()} sats = $${usdRounded.toFixed(2)} USD (rate: $${btcPriceUsd.toLocaleString()}/BTC)`,
  );
  console.log(JSON.stringify(output, null, 2));
}

async function cmdUsdToSats(amountUsd, apiKey, apiUrl) {
  // Single API call: ask "how many sats is $<amountUsd> worth?" and derive rate mathematically
  const data = await graphqlRequest({
    query: CONVERSION_QUERY,
    variables: { amount: amountUsd, currency: 'USD' },
    apiKey,
    apiUrl,
  });
  const est = data.currencyConversionEstimation;
  const sats = est.btcSatAmount;

  // Derive satsPerDollar from the same response instead of a second API call
  const satsPerDollar = amountUsd > 0 ? Math.round(sats / amountUsd) : 0;
  const btcPriceUsd = satsPerDollar > 0 ? Math.round((100_000_000 / satsPerDollar) * 100) / 100 : 0;

  const output = {
    btcPriceUsd,
    satsPerDollar,
    conversion: {
      usd: amountUsd,
      usdFormatted: `$${amountUsd.toFixed(2)}`,
      sats,
    },
  };

  console.error(
    `$${amountUsd.toFixed(2)} USD = ${sats.toLocaleString()} sats (rate: $${btcPriceUsd.toLocaleString()}/BTC)`,
  );
  console.log(JSON.stringify(output, null, 2));
}

async function cmdHistory(range, apiKey, apiUrl) {
  const validRanges = ['ONE_DAY', 'ONE_WEEK', 'ONE_MONTH', 'ONE_YEAR', 'FIVE_YEARS'];
  if (!validRanges.includes(range)) {
    console.error(`Error: invalid range "${range}". Valid: ${validRanges.join(', ')}`);
    process.exit(1);
  }

  const data = await graphqlRequest({
    query: BTC_PRICE_LIST_QUERY,
    variables: { range },
    apiKey,
    apiUrl,
  });
  const prices = data.btcPriceList;

  // Convert base/offset to readable USD prices
  const points = prices.map((p) => {
    const priceUsd = p.price.base / Math.pow(10, p.price.offset) / 100; // offset gives cents
    return {
      timestamp: p.timestamp,
      date: new Date(p.timestamp * 1000).toISOString(),
      btcPriceUsd: Math.round(priceUsd * 100) / 100,
      formatted: p.price.formattedAmount || `$${(Math.round(priceUsd * 100) / 100).toLocaleString()}`,
    };
  });

  // Summary stats
  const usdPrices = points.map((p) => p.btcPriceUsd);
  const current = usdPrices[usdPrices.length - 1] || 0;
  const oldest = usdPrices[0] || 0;
  const high = Math.max(...usdPrices);
  const low = Math.min(...usdPrices);
  const changeUsd = Math.round((current - oldest) * 100) / 100;
  const changePct = oldest > 0 ? Math.round(((current - oldest) / oldest) * 10000) / 100 : 0;

  const output = {
    range,
    dataPoints: points.length,
    summary: {
      current,
      oldest,
      high,
      low,
      changeUsd,
      changePct,
    },
    prices: points,
  };

  console.error(
    `BTC price history (${range}): $${oldest.toLocaleString()} → $${current.toLocaleString()} (${changeUsd >= 0 ? '+' : ''}${changeUsd} / ${changePct}%)`,
  );
  console.error(`Range: $${low.toLocaleString()} – $${high.toLocaleString()} across ${points.length} data points`);
  console.log(JSON.stringify(output, null, 2));
}

async function cmdCurrencies(apiKey, apiUrl) {
  const data = await graphqlRequest({ query: CURRENCY_LIST_QUERY, apiKey, apiUrl });
  const currencies = data.currencyList;

  const output = {
    count: currencies.length,
    currencies: currencies.map((c) => ({
      id: c.id,
      name: c.name,
      symbol: c.symbol,
      flag: c.flag,
      fractionDigits: c.fractionDigits,
    })),
  };

  console.error(`${currencies.length} supported currencies`);
  console.log(JSON.stringify(output, null, 2));
}

// ── Main ────────────────────────────────────────────────────

async function main() {
  const apiKey = getApiKey({ required: false });
  const apiUrl = getApiUrl();
  const args = process.argv.slice(2);

  // Check for --raw flag
  const includeRaw = args.includes('--raw');
  const filteredArgs = args.filter((a) => a !== '--raw');

  if (filteredArgs.length === 0) {
    // No args: current BTC/USD price
    return cmdCurrentPrice(apiKey, apiUrl, includeRaw);
  }

  if (filteredArgs[0] === '--usd') {
    const amount = parseFloat(filteredArgs[1]);
    if (isNaN(amount) || amount < 0) {
      console.error('Usage: node price.js --usd <amount_usd>');
      console.error('  amount_usd: USD amount to convert to sats (e.g. 1.50)');
      process.exit(1);
    }
    return cmdUsdToSats(amount, apiKey, apiUrl);
  }

  if (filteredArgs[0] === '--history') {
    const range = (filteredArgs[1] || '').toUpperCase();
    if (!range) {
      console.error('Usage: node price.js --history <range>');
      console.error('  range: ONE_DAY, ONE_WEEK, ONE_MONTH, ONE_YEAR, FIVE_YEARS');
      process.exit(1);
    }
    return cmdHistory(range, apiKey, apiUrl);
  }

  if (filteredArgs[0] === '--currencies') {
    return cmdCurrencies(apiKey, apiUrl);
  }

  // Default: treat first arg as sat amount for conversion
  const amountSats = parseInt(filteredArgs[0], 10);
  if (isNaN(amountSats) || amountSats < 0) {
    console.error('Usage: node price.js [amount_sats]');
    console.error('       node price.js --usd <amount_usd>');
    console.error('       node price.js --history <range>');
    console.error('       node price.js --currencies');
    console.error('       node price.js --raw');
    process.exit(1);
  }
  return cmdSatsToUsd(amountSats, apiKey, apiUrl);
}

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
