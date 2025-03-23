import * as Crypto from "expo-crypto"

// Define the types for the crypto polyfill
interface CryptoPolyfill {
  getRandomValues: (array: Uint8Array) => Uint8Array;
  subtle?: SubtleCrypto;
  randomUUID?: () => string;
}

// Polyfill crypto.getRandomValues
if (typeof crypto === 'undefined') {
  const cryptoPolyfill: CryptoPolyfill = {
    getRandomValues: (array: Uint8Array): Uint8Array => {
      const randomBytes = Crypto.getRandomValues(array);
      if (randomBytes) {
        array.set(new Uint8Array(randomBytes));
      }
      return array;
    },
    // Add stub implementations for required crypto methods
    subtle: undefined,
    randomUUID: () => {
      // Simple UUID v4 implementation
      return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
        const r = Math.random() * 16 | 0;
        const v = c === 'x' ? r : (r & 0x3 | 0x8);
        return v.toString(16);
      });
    }
  };

  (global as any).crypto = cryptoPolyfill;
}
