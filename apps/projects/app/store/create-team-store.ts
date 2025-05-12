import { create } from 'zustand';

interface CreateTeamStore {
  isOpen: boolean;
  openModal: () => void;
  closeModal: () => void;
}

export const useCreateTeamStore = create<CreateTeamStore>((set) => ({
  isOpen: false,
  openModal: () => set({ isOpen: true }),
  closeModal: () => set({ isOpen: false }),
}));