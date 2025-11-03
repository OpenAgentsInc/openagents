import { create } from 'zustand/traditional'

type UpdateState = {
  updating: boolean;
  setUpdating: (v: boolean) => void;
}

export const useUpdateStore = create<UpdateState>((set) => ({
  updating: false,
  setUpdating: (v: boolean) => set({ updating: v }),
}))
