#!/usr/bin/env node
/**
 * Blink Wallet - Subscribe to Account Updates (myUpdates)
 *
 * Usage: node --experimental-websocket subscribe_updates.js [--timeout <seconds>] [--max <count>]
 *
 * Streams account activity updates via Blink's GraphQL WebSocket API. Each
 * event is printed as a JSON line to stdout (NDJSON). Status messages go to
 * stderr. Use --timeout 0 for no timeout.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
 */

const { getApiKey, getWsUrl, requireWebSocket } = require('./_blink_client');

function parseArgs(argv) {
  let timeoutSeconds = 0;
  let maxCount = 0;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--timeout') {
      const value = argv[i + 1];
      if (!value) throw new Error('Missing value for --timeout');
      timeoutSeconds = parseInt(value, 10);
      if (isNaN(timeoutSeconds) || timeoutSeconds < 0) throw new Error('--timeout must be a non-negative integer');
      i++;
    } else if (arg === '--max') {
      const value = argv[i + 1];
      if (!value) throw new Error('Missing value for --max');
      maxCount = parseInt(value, 10);
      if (isNaN(maxCount) || maxCount < 0) throw new Error('--max must be a non-negative integer');
      i++;
    }
  }

  return { timeoutSeconds, maxCount };
}

const MY_UPDATES_SUBSCRIPTION = `subscription myUpdates {
  myUpdates {
    update {
      __typename
      ... on LnUpdate {
        transaction {
          initiationVia {
            ... on InitiationViaLn {
              paymentHash
            }
          }
          direction
          settlementAmount
          settlementCurrency
          status
          createdAt
          memo
        }
      }
      ... on OnChainUpdate {
        transaction {
          direction
          settlementAmount
          settlementCurrency
          status
          createdAt
          memo
        }
      }
      ... on IntraLedgerUpdate {
        transaction {
          direction
          settlementAmount
          settlementCurrency
          status
          createdAt
          memo
        }
      }
    }
  }
}`;

function main() {
  const args = parseArgs(process.argv.slice(2));
  const apiKey = getApiKey();
  const wsUrl = getWsUrl();
  const WebSocketImpl = requireWebSocket();

  let done = false;
  let count = 0;
  let timeoutId = null;

  const ws = new WebSocketImpl(wsUrl, 'graphql-transport-ws');

  function finish(exitCode = 0) {
    if (done) return;
    done = true;
    if (timeoutId) clearTimeout(timeoutId);
    try {
      if (ws.readyState === ws.OPEN) ws.send(JSON.stringify({ id: '1', type: 'complete' }));
    } catch {
      // best-effort cleanup
    }
    try {
      ws.close(1000);
    } catch {
      // best-effort cleanup
    }
    process.exit(exitCode);
  }

  if (args.timeoutSeconds > 0) {
    timeoutId = setTimeout(() => {
      console.error('Timeout reached, exiting.');
      finish(0);
    }, args.timeoutSeconds * 1000);
  }

  ws.onopen = () => {
    ws.send(
      JSON.stringify({
        type: 'connection_init',
        payload: { 'X-API-KEY': apiKey },
      }),
    );
  };

  ws.onmessage = (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch {
      console.error('Warning: received non-JSON message');
      return;
    }

    if (message.type === 'connection_ack') {
      ws.send(
        JSON.stringify({
          id: '1',
          type: 'subscribe',
          payload: {
            query: MY_UPDATES_SUBSCRIPTION,
            variables: {},
          },
        }),
      );
      console.error('Subscribed to myUpdates.');
      return;
    }

    if (message.type === 'ping') {
      ws.send(JSON.stringify({ type: 'pong' }));
      return;
    }

    if (message.type === 'error') {
      console.error('Error: subscription error', JSON.stringify(message.payload || message));
      finish(1);
      return;
    }

    if (message.type === 'next') {
      const update =
        message.payload && message.payload.data && message.payload.data.myUpdates
          ? message.payload.data.myUpdates.update
          : null;
      if (!update || Object.keys(update).length === 0) return;

      const out = {
        type: update.__typename || null,
        receivedAt: new Date().toISOString(),
        update,
      };
      process.stdout.write(`${JSON.stringify(out)}\n`);

      count++;
      if (args.maxCount > 0 && count >= args.maxCount) {
        console.error(`Max count ${args.maxCount} reached, exiting.`);
        finish(0);
      }
      return;
    }

    if (message.type === 'complete') {
      finish(0);
    }
  };

  ws.onerror = () => {
    console.error('Error: WebSocket error');
    finish(1);
  };

  ws.onclose = (event) => {
    if (done) return;
    console.error(`WebSocket closed: code=${event.code} reason=${event.reason || 'unknown'}`);
    finish(1);
  };
}

try {
  main();
} catch (e) {
  console.error('Error:', e.message);
  process.exit(1);
}
