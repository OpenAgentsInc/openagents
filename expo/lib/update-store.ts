import { create } from 'zustand'

type UpdateState = {
  updating: boolean;
  // Temporary: force showing the updating screen for visual review
  forceOverlay: boolean;
  setUpdating: (v: boolean) => void;
  setForceOverlay: (v: boolean) => void;
}

export const useUpdateStore = create<UpdateState>((set) => ({
  updating: false,
  // TEMP: hardcode true so the updating screen is visible for review
  forceOverlay: true,
  setUpdating: (v: boolean) => set({ updating: v }),
  setForceOverlay: (v: boolean) => set({ forceOverlay: v }),
}))

