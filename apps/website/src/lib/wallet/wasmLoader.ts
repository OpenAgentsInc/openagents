import initBreezSDK from "@breeztech/breez-sdk-spark";
import { walletLogger, LogCategory } from "./logger";

let initialized = false;

export const initWasm = async (): Promise<void> => {
  if (initialized) return;
  try {
    walletLogger.info(LogCategory.SDK, "Initializing WASM module");
    await initBreezSDK();
    walletLogger.info(LogCategory.SDK, "WASM module initialized successfully");
    initialized = true;
  } catch (error) {
    walletLogger.error(LogCategory.SDK, "Failed to initialize WASM module", {
      error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }
};

export const isWasmInitialized = (): boolean => initialized;
