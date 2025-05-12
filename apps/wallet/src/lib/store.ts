import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

export type WalletState = 'login' | 'creating_disclaimer' | 'showing_mnemonic' | 'entering_seed' | 'initializing_wallet' | 'wallet_ready' | 'error';

interface WalletStoreState {
  // App state
  appState: WalletState;
  setAppState: (state: WalletState) => void;
  
  // Wallet mnemonic management
  mnemonic: string | null;
  setMnemonic: (mnemonic: string | null) => void;
  
  // Error handling
  errorMessage: string | null;
  setErrorMessage: (message: string | null) => void;
  
  // Wallet reset
  resetWallet: () => void;
}

export const useWalletStore = create<WalletStoreState>()(
  persist(
    (set) => ({
      appState: 'login',
      setAppState: (state) => set({ appState: state }),
      
      mnemonic: null,
      setMnemonic: (mnemonic) => set({ mnemonic }),
      
      errorMessage: null,
      setErrorMessage: (message) => set({ errorMessage: message }),
      
      resetWallet: () => set({
        appState: 'login',
        mnemonic: null,
        errorMessage: null
      })
    }),
    {
      name: 'openagents-wallet-storage',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ 
        // Only persist the mnemonic, not app state or errors
        mnemonic: state.mnemonic 
      })
    }
  )
)