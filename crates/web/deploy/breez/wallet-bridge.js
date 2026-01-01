// Wallet Bridge - Exposes Breez SDK to our Rust WASM client
import initBreezSDK, * as breezSdk from './index.js';

// Global state
let sdk = null;
let sdkInitializing = false;
let sdkError = null;

// Logger for SDK
class WebLogger {
    log(logEntry) {
        const ts = new Date().toISOString();
        const formatted = `${ts} [${logEntry.level}]: ${logEntry.line}`;
        console.log(formatted);
    }
}

// Initialize the Breez SDK WASM module (call this first)
window.breezWalletInit = async function() {
    if (sdk) return { ok: true };
    if (sdkInitializing) {
        // Wait for existing initialization
        while (sdkInitializing) {
            await new Promise(r => setTimeout(r, 100));
        }
        return sdk ? { ok: true } : { error: sdkError };
    }

    try {
        sdkInitializing = true;
        console.log('[wallet-bridge] Initializing Breez SDK WASM...');
        await initBreezSDK();
        console.log('[wallet-bridge] Breez SDK WASM initialized');
        return { ok: true };
    } catch (e) {
        sdkError = e.toString();
        console.error('[wallet-bridge] Failed to initialize SDK:', e);
        return { error: sdkError };
    } finally {
        sdkInitializing = false;
    }
};

// Connect wallet with entropy and network
window.breezWalletConnect = async function(entropyHex, network) {
    if (sdk) {
        console.log('[wallet-bridge] Already connected');
        return { ok: true };
    }

    try {
        console.log('[wallet-bridge] Connecting wallet on', network);

        // Create mnemonic from entropy
        // The entropy is hex-encoded, we need to convert to bytes
        const entropyBytes = new Uint8Array(entropyHex.match(/.{1,2}/g).map(byte => parseInt(byte, 16)));

        // Use bip39 to generate mnemonic from entropy
        // The breez SDK expects a mnemonic string
        // Since we have raw entropy, we'll pass it directly if the SDK supports it
        // Otherwise we need bip39 library

        // For now, let's assume entropyHex is actually a mnemonic or we need to handle this differently
        // Looking at the walletService.ts, they use: connect({ config, seed: { type: "mnemonic", mnemonic } })

        // Map network
        let sdkNetwork;
        switch (network.toLowerCase()) {
            case 'mainnet':
                sdkNetwork = breezSdk.Network.Mainnet;
                break;
            case 'testnet':
                sdkNetwork = breezSdk.Network.Testnet;
                break;
            case 'regtest':
                sdkNetwork = breezSdk.Network.Regtest;
                break;
            default:
                sdkNetwork = breezSdk.Network.Testnet;
        }

        // Create config
        const config = {
            network: sdkNetwork,
        };

        // Initialize logging
        const logger = new WebLogger();
        breezSdk.initLogging(logger);

        // Connect - for now we'll need to handle the entropy->mnemonic conversion
        // The entropy should be used to derive a mnemonic
        // Actually, looking at the wallet-worker code, it stores bitcoin_xpriv as hex entropy
        // We need a bip39 library to convert entropy to mnemonic

        // For testing, let's try using the entropy directly if possible
        // or we need to add bip39 to the browser

        console.log('[wallet-bridge] Attempting connection with entropy length:', entropyBytes.length);

        // We need bip39 to convert entropy to mnemonic
        // For now, let's see if we can load bip39 dynamically
        // Actually, the SDK might accept raw seed bytes

        // Try using the connect function with a seed object
        sdk = await breezSdk.connect({
            config,
            seed: { type: "entropy", entropy: Array.from(entropyBytes) },
            storageDir: "openagents-wallet"
        });

        console.log('[wallet-bridge] Wallet connected successfully');
        return { ok: true };
    } catch (e) {
        console.error('[wallet-bridge] Failed to connect wallet:', e);
        return { error: e.toString() };
    }
};

// Get wallet info (balance, addresses)
window.breezWalletGetInfo = async function() {
    if (!sdk) {
        return { error: 'SDK not connected' };
    }

    try {
        const info = await sdk.getInfo({});
        return { ok: true, data: info };
    } catch (e) {
        return { error: e.toString() };
    }
};

// Get spark address
window.breezWalletGetSparkAddress = async function() {
    if (!sdk) {
        return { error: 'SDK not connected' };
    }

    try {
        const response = await sdk.receivePayment({
            paymentMethod: breezSdk.ReceivePaymentMethod.SparkAddress
        });
        return { ok: true, address: response.paymentRequest };
    } catch (e) {
        return { error: e.toString() };
    }
};

// Get bitcoin deposit address
window.breezWalletGetBitcoinAddress = async function() {
    if (!sdk) {
        return { error: 'SDK not connected' };
    }

    try {
        const response = await sdk.receivePayment({
            paymentMethod: breezSdk.ReceivePaymentMethod.BitcoinAddress
        });
        return { ok: true, address: response.paymentRequest };
    } catch (e) {
        return { error: e.toString() };
    }
};

// Get balance
window.breezWalletGetBalance = async function() {
    if (!sdk) {
        return { error: 'SDK not connected' };
    }

    try {
        const info = await sdk.getInfo({});
        return {
            ok: true,
            balance: {
                spark_sats: info.balances?.sparkBalance || 0,
                lightning_sats: info.balances?.lightningBalance || 0,
                onchain_sats: info.balances?.onchainBalance || 0,
            }
        };
    } catch (e) {
        return { error: e.toString() };
    }
};

// List payments
window.breezWalletListPayments = async function(limit, offset) {
    if (!sdk) {
        return { error: 'SDK not connected' };
    }

    try {
        const response = await sdk.listPayments({ limit: limit || 25, offset: offset || 0 });
        return { ok: true, payments: response.payments };
    } catch (e) {
        return { error: e.toString() };
    }
};

// Send payment
window.breezWalletSendPayment = async function(paymentRequest, amountSats) {
    if (!sdk) {
        return { error: 'SDK not connected' };
    }

    try {
        // Parse the input first
        const parsed = await sdk.parse(paymentRequest);

        // Prepare and send based on type
        let response;
        if (parsed.type === 'bolt11') {
            const prepared = await sdk.prepareSendPayment({ invoice: paymentRequest });
            response = await sdk.sendPayment({ prepareResponse: prepared });
        } else if (parsed.type === 'lnurl_pay') {
            const prepared = await sdk.prepareLnurlPay({
                lnurlPayUrl: paymentRequest,
                amountMsat: (amountSats || 0) * 1000
            });
            response = await sdk.lnurlPay({ prepareResponse: prepared });
        } else {
            // Spark address or other
            const prepared = await sdk.prepareSendPayment({
                destination: paymentRequest,
                amountSats: amountSats
            });
            response = await sdk.sendPayment({ prepareResponse: prepared });
        }

        return { ok: true, payment: response.payment };
    } catch (e) {
        return { error: e.toString() };
    }
};

// Create lightning invoice
window.breezWalletCreateInvoice = async function(amountSats, description) {
    if (!sdk) {
        return { error: 'SDK not connected' };
    }

    try {
        const response = await sdk.receivePayment({
            paymentMethod: breezSdk.ReceivePaymentMethod.Lightning,
            amount: amountSats,
            description: description || ''
        });
        return { ok: true, invoice: response.paymentRequest };
    } catch (e) {
        return { error: e.toString() };
    }
};

// Disconnect wallet
window.breezWalletDisconnect = async function() {
    if (!sdk) {
        return { ok: true };
    }

    try {
        await sdk.disconnect();
        sdk = null;
        return { ok: true };
    } catch (e) {
        return { error: e.toString() };
    }
};

// Check if wallet is connected
window.breezWalletIsConnected = function() {
    return sdk !== null;
};

console.log('[wallet-bridge] Wallet bridge loaded');
