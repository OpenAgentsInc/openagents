// Web/Browser entry point for Breez SDK with automatic IndexedDB storage support
import wasmInit, * as wasmModule from './breez_sdk_spark_wasm.js';

// Automatically import and set up the IndexedDB storage for web/browser environments
let storageSetupComplete = false;

const setupWebStorage = async () => {
    if (storageSetupComplete) return;
    
    try {
        // Dynamic import of storage module
        const { createDefaultStorage } = await import('./storage/index.js');
        
        // Make createDefaultStorage available globally for WASM to find
        globalThis.createDefaultStorage = createDefaultStorage;
        
        console.log('Breez SDK: Web IndexedDB storage automatically enabled');
        storageSetupComplete = true;
    } catch (error) {
        console.warn('Breez SDK: Failed to load Web storage:', error.message);
        console.warn('Breez SDK: Storage operations may not work properly. Ignore this warning if you are not using the default storage.');
    }
};

// Initialize WASM and storage
const initBreezSDK = async () => {
    await setupWebStorage();
    return await wasmInit();
};

// Export the initialization function and all WASM functions
export default initBreezSDK;
export * from './breez_sdk_spark_wasm.js';
