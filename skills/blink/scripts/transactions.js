#!/usr/bin/env node
/**
 * Blink Wallet - List Transactions
 *
 * Usage: node transactions.js [--first N] [--after CURSOR] [--wallet BTC|USD]
 *
 * Lists recent transactions (incoming and outgoing) with pagination support.
 * Returns JSON array of transactions with direction, amount, status, and metadata.
 *
 * Options:
 *   --first N       - Number of transactions to return (default: 20, max: 100)
 *   --after CURSOR  - Pagination cursor (from previous response's endCursor)
 *   --wallet BTC|USD - Filter to a specific wallet currency (default: all)
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 *
 * Dependencies: None (uses Node.js built-in fetch)
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const DEFAULT_API_URL = 'https://api.blink.sv/graphql';

function getApiKey() {
  let key = process.env.BLINK_API_KEY;
  if (!key) {
    try {
      const profile = fs.readFileSync(path.join(os.homedir(), '.profile'), 'utf8');
      const match = profile.match(/BLINK_API_KEY=["']?([a-zA-Z0-9_]+)["']?/);
      if (match) key = match[1];
    } catch {}
  }
  if (!key) throw new Error('BLINK_API_KEY not found. Set it in environment or ~/.profile');
  return key;
}

function getApiUrl() {
  return process.env.BLINK_API_URL || DEFAULT_API_URL;
}

async function graphqlRequest(query, variables = {}) {
  const apiKey = getApiKey();
  const apiUrl = getApiUrl();

  const res = await fetch(apiUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-API-KEY': apiKey,
    },
    body: JSON.stringify({ query, variables }),
  });

  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${await res.text()}`);
  }

  const json = await res.json();
  if (json.errors && json.errors.length > 0) {
    throw new Error(`GraphQL error: ${json.errors.map(e => e.message).join(', ')}`);
  }
  return json.data;
}

const WALLET_QUERY = `
  query Me {
    me {
      defaultAccount {
        wallets {
          id
          walletCurrency
        }
      }
    }
  }
`;

const TRANSACTIONS_QUERY = `
  query Transactions($first: Int, $after: String, $walletIds: [WalletId]) {
    me {
      defaultAccount {
        transactions(first: $first, after: $after, walletIds: $walletIds) {
          pageInfo {
            hasNextPage
            hasPreviousPage
            startCursor
            endCursor
          }
          edges {
            cursor
            node {
              id
              direction
              status
              memo
              createdAt
              settlementAmount
              settlementCurrency
              settlementDisplayAmount
              settlementDisplayCurrency
              settlementFee
              initiationVia {
                ... on InitiationViaLn {
                  paymentHash
                  paymentRequest
                }
                ... on InitiationViaOnChain {
                  address
                }
                ... on InitiationViaIntraLedger {
                  counterPartyUsername
                  counterPartyWalletId
                }
              }
              settlementVia {
                ... on SettlementViaLn {
                  preImage
                }
                ... on SettlementViaOnChain {
                  transactionHash
                  vout
                }
                ... on SettlementViaIntraLedger {
                  counterPartyUsername
                  counterPartyWalletId
                  preImage
                }
              }
            }
          }
        }
      }
    }
  }
`;

function parseArgs(argv) {
  const args = { first: 20, after: null, wallet: null };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--first' && argv[i + 1]) {
      args.first = parseInt(argv[++i], 10);
      if (isNaN(args.first) || args.first < 1) args.first = 20;
      if (args.first > 100) args.first = 100;
    } else if (argv[i] === '--after' && argv[i + 1]) {
      args.after = argv[++i];
    } else if (argv[i] === '--wallet' && argv[i + 1]) {
      args.wallet = argv[++i].toUpperCase();
      if (args.wallet !== 'BTC' && args.wallet !== 'USD') {
        console.error('Error: --wallet must be BTC or USD');
        process.exit(1);
      }
    }
  }
  return args;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  // Resolve wallet IDs if filtering
  let walletIds = null;
  if (args.wallet) {
    const walletData = await graphqlRequest(WALLET_QUERY);
    if (!walletData.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');
    const wallet = walletData.me.defaultAccount.wallets.find(w => w.walletCurrency === args.wallet);
    if (!wallet) throw new Error(`No ${args.wallet} wallet found on this account.`);
    walletIds = [wallet.id];
  }

  const variables = { first: args.first };
  if (args.after) variables.after = args.after;
  if (walletIds) variables.walletIds = walletIds;

  const data = await graphqlRequest(TRANSACTIONS_QUERY, variables);
  if (!data.me) throw new Error('Authentication failed. Check your BLINK_API_KEY.');

  const connection = data.me.defaultAccount.transactions;
  if (!connection) {
    console.log(JSON.stringify({ transactions: [], pageInfo: null }, null, 2));
    return;
  }

  const transactions = connection.edges.map(edge => {
    const tx = edge.node;
    const result = {
      id: tx.id,
      direction: tx.direction,
      status: tx.status,
      amount: tx.settlementAmount,
      currency: tx.settlementCurrency,
      displayAmount: tx.settlementDisplayAmount,
      displayCurrency: tx.settlementDisplayCurrency,
      fee: tx.settlementFee,
      memo: tx.memo || null,
      createdAt: tx.createdAt,
    };

    // Flatten initiation via
    if (tx.initiationVia) {
      if (tx.initiationVia.paymentHash) {
        result.type = 'lightning';
        result.paymentHash = tx.initiationVia.paymentHash;
      } else if (tx.initiationVia.address) {
        result.type = 'onchain';
        result.onchainAddress = tx.initiationVia.address;
      } else if (tx.initiationVia.counterPartyUsername) {
        result.type = 'intraledger';
        result.counterParty = tx.initiationVia.counterPartyUsername;
      } else {
        result.type = 'unknown';
      }
    }

    // Flatten settlement via
    if (tx.settlementVia) {
      if (tx.settlementVia.transactionHash) {
        result.onchainTxHash = tx.settlementVia.transactionHash;
      }
      if (tx.settlementVia.preImage) {
        result.preImage = tx.settlementVia.preImage;
      }
    }

    return result;
  });

  const output = {
    transactions,
    count: transactions.length,
    pageInfo: {
      hasNextPage: connection.pageInfo.hasNextPage,
      endCursor: connection.pageInfo.endCursor,
    },
  };

  console.log(JSON.stringify(output, null, 2));
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});