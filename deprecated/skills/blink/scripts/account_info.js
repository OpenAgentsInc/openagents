#!/usr/bin/env node
/**
 * Blink Wallet - Account Info
 *
 * Usage: node account_info.js
 *
 * Shows account level, spending limits (withdrawal, internal send, convert),
 * default wallet, and wallet summary. Useful for understanding constraints
 * before attempting operations that might exceed limits.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const { getApiKey, getApiUrl, graphqlRequest, estimateSatsToUsd } = require('./_blink_client');

const ACCOUNT_INFO_QUERY = `
  query AccountInfo {
    me {
      id
      username
      defaultAccount {
        id
        level
        defaultWallet {
          id
          walletCurrency
        }
        wallets {
          id
          walletCurrency
          balance
          pendingIncomingBalance
        }
        limits {
          withdrawal {
            ... on OneDayAccountLimit {
              totalLimit
              remainingLimit
              interval
            }
          }
          internalSend {
            ... on OneDayAccountLimit {
              totalLimit
              remainingLimit
              interval
            }
          }
          convert {
            ... on OneDayAccountLimit {
              totalLimit
              remainingLimit
              interval
            }
          }
        }
      }
    }
  }
`;

function formatLimit(limits) {
  if (!limits || limits.length === 0) return null;
  return limits.map((l) => ({
    totalLimitCents: l.totalLimit,
    remainingLimitCents: l.remainingLimit,
    totalLimitUsd: `$${(l.totalLimit / 100).toFixed(2)}`,
    remainingLimitUsd: l.remainingLimit !== null ? `$${(l.remainingLimit / 100).toFixed(2)}` : null,
    intervalSeconds: l.interval,
    intervalHours: l.interval ? Math.round(l.interval / 3600) : null,
  }));
}

async function main() {
  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  const data = await graphqlRequest({ query: ACCOUNT_INFO_QUERY, apiKey, apiUrl });
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');

  const me = data.me;
  const account = me.defaultAccount;

  const walletEntries = [];
  for (const w of account.wallets) {
    const entry = {
      id: w.id,
      currency: w.walletCurrency,
      balance: w.balance,
      pendingIncoming: w.pendingIncomingBalance,
      unit: w.walletCurrency === 'BTC' ? 'sats' : 'cents',
    };
    if (w.walletCurrency === 'BTC') {
      const usdEstimate = await estimateSatsToUsd({ sats: w.balance, apiKey, apiUrl });
      if (usdEstimate !== null) {
        entry.balanceUsd = usdEstimate;
        entry.balanceUsdFormatted = `$${usdEstimate.toFixed(2)}`;
      }
    }
    if (w.walletCurrency === 'USD') {
      entry.balanceUsdFormatted = `$${(w.balance / 100).toFixed(2)}`;
    }
    walletEntries.push(entry);
  }

  const output = {
    userId: me.id,
    username: me.username || null,
    accountId: account.id,
    accountLevel: account.level,
    defaultWallet: {
      id: account.defaultWallet.id,
      currency: account.defaultWallet.walletCurrency,
    },
    wallets: walletEntries,
    limits: {
      withdrawal: formatLimit(account.limits.withdrawal),
      internalSend: formatLimit(account.limits.internalSend),
      convert: formatLimit(account.limits.convert),
    },
  };

  console.log(JSON.stringify(output, null, 2));
}

main().catch((e) => {
  console.error('Error:', e.message);
  process.exit(1);
});
