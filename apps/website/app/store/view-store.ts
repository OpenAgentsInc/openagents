import { create } from 'zustand';

export type ViewType = 'list' | 'grid';

interface ViewState {
   viewType: ViewType;
   setViewType: (viewType: ViewType) => void;
}

export const useViewStore = create<ViewState>((set) => ({
   viewType: 'list',
   setViewType: (viewType: ViewType) => set({ viewType }),
}));
