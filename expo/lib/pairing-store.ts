import { create } from 'zustand'

type PairingState = {
  deeplinkPairing: boolean;
  setDeeplinkPairing: (v: boolean) => void;
}

export const usePairingStore = create<PairingState>((set) => ({
  deeplinkPairing: false,
  setDeeplinkPairing: (v: boolean) => set({ deeplinkPairing: v }),
}))

