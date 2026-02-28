#!/usr/bin/env node
/**
 * Blink Wallet - Subscribe to Invoice Payment Status
 *
 * Usage: node --experimental-websocket subscribe_invoice.js <bolt11_invoice> [--timeout <seconds>]
 *
 * Watches a Lightning invoice via Blink's GraphQL WebSocket API and exits when
 * the invoice is PAID or EXPIRED. Status updates are printed to stderr, final
 * JSON result to stdout.
 *
 * Arguments:
 *   bolt11_invoice - Required. The BOLT-11 payment request string (lnbc...)
 *   --timeout      - Optional. Timeout in seconds (default: 300). Use 0 for no timeout.
 *
 * Environment:
 *   BLINK_API_KEY  - Required. Blink API key (format: blink_...)
 *   BLINK_API_URL  - Optional. Override API endpoint (default: https://api.blink.sv/graphql)
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

function getWsUrl() {
  const apiUrl = getApiUrl();
  const url = new URL(apiUrl);
  url.protocol = url.protocol === 'http:' ? 'ws:' : 'wss:';
  if (url.hostname.startsWith('api.')) url.hostname = url.hostname.replace(/^api\./, 'ws.');
  return url.toString();
}

function normalizeInvoice(input) {
  const trimmed = input.trim();
  if (trimmed.toLowerCase().startsWith('lightning:')) return trimmed.slice('lightning:'.length);
  return trimmed;
}

function parseArgs(argv) {
  let invoice = null;
  let timeoutSeconds = 300;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--timeout') {
      const value = argv[i + 1];
      if (!value) throw new Error('Missing value for --timeout');
      timeoutSeconds = parseInt(value, 10);
      if (isNaN(timeoutSeconds) || timeoutSeconds < 0) throw new Error('--timeout must be a non-negative integer');
      i++;
    } else if (!invoice) {
      invoice = arg;
    }
  }

  return { invoice, timeoutSeconds };
}

function requireWebSocket() {
  if (typeof WebSocket !== 'function') {
    throw new Error('WebSocket is not available. Run with: node --experimental-websocket subscribe_invoice.js ...');
  }
  return WebSocket;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.invoice) {
    console.error('Usage: node --experimental-websocket subscribe_invoice.js <bolt11_invoice> [--timeout <seconds>]');
    process.exit(1);
  }

  const paymentRequest = normalizeInvoice(args.invoice);
  if (!paymentRequest) {
    console.error('Error: bolt11_invoice must be a non-empty string');
    process.exit(1);
  }

  const apiKey = getApiKey();
  const wsUrl = getWsUrl();
  const WebSocketImpl = requireWebSocket();

  let done = false;
  let timeoutId = null;

  const ws = new WebSocketImpl(wsUrl, 'graphql-transport-ws');

  function finish(result, exitCode = 0) {
    if (done) return;
    done = true;
    if (timeoutId) clearTimeout(timeoutId);
    try {
      if (ws.readyState === ws.OPEN) {
        ws.send(JSON.stringify({ id: '1', type: 'complete' }));
      }
    } catch {}
    try {
      ws.close(1000);
    } catch {}
    if (result) {
      console.log(JSON.stringify(result, null, 2));
    }
    process.exit(exitCode);
  }

  if (args.timeoutSeconds > 0) {
    timeoutId = setTimeout(() => {
      finish({
        paymentRequest,
        status: 'TIMEOUT',
        isPaid: false,
        isExpired: false,
        isPending: true,
      }, 1);
    }, args.timeoutSeconds * 1000);
  }

  ws.onopen = () => {
    ws.send(JSON.stringify({
      type: 'connection_init',
      payload: { 'X-API-KEY': apiKey },
    }));
  };

  ws.onmessage = (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch (e) {
      console.error('Warning: received non-JSON message');
      return;
    }

    if (message.type === 'connection_ack') {
      ws.send(JSON.stringify({
        id: '1',
        type: 'subscribe',
        payload: {
          query: `subscription LnInvoicePaymentStatus($input: LnInvoicePaymentStatusInput!) {\n  lnInvoicePaymentStatus(input: $input) {\n    status\n  }\n}`,
          variables: { input: { paymentRequest } },
        },
      }));
      return;
    }

    if (message.type === 'ping') {
      ws.send(JSON.stringify({ type: 'pong' }));
      return;
    }

    if (message.type === 'error') {
      console.error('Error: subscription error', JSON.stringify(message.payload || message));
      finish({
        paymentRequest,
        status: 'ERROR',
        error: message.payload || message,
      }, 1);
      return;
    }

    if (message.type === 'next') {
      const status = message.payload && message.payload.data && message.payload.data.lnInvoicePaymentStatus
        ? message.payload.data.lnInvoicePaymentStatus.status
        : null;
      if (!status) return;

      console.error(`Status: ${status}`);
      if (status === 'PAID' || status === 'EXPIRED') {
        finish({
          paymentRequest,
          status,
          isPaid: status === 'PAID',
          isExpired: status === 'EXPIRED',
          isPending: false,
        }, 0);
      }
      return;
    }

    if (message.type === 'complete') {
      finish({
        paymentRequest,
        status: 'COMPLETE',
        isPaid: false,
        isExpired: false,
        isPending: true,
      }, 0);
    }
  };

  ws.onerror = (event) => {
    console.error('Error: WebSocket error');
    finish({
      paymentRequest,
      status: 'ERROR',
      error: 'WebSocket error',
    }, 1);
  };

  ws.onclose = (event) => {
    if (done) return;
    console.error(`WebSocket closed: code=${event.code} reason=${event.reason || 'unknown'}`);
    finish({
      paymentRequest,
      status: 'CLOSED',
      isPaid: false,
      isExpired: false,
      isPending: true,
    }, 1);
  };
}

try {
  main();
} catch (e) {
  console.error('Error:', e.message);
  process.exit(1);
}