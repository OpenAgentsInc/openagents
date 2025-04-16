import { create } from 'zustand';

interface CreateProjectStore {
  isOpen: boolean;
  openModal: () => void;
  closeModal: () => void;
}

export const useCreateProjectStore = create<CreateProjectStore>((set) => ({
  isOpen: false,
  openModal: () => set({ isOpen: true }),
  closeModal: () => set({ isOpen: false }),
}));