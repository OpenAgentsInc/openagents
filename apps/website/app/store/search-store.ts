import { create } from 'zustand';

interface SearchState {
   isSearchOpen: boolean;
   searchQuery: string;

   openSearch: () => void;
   closeSearch: () => void;
   toggleSearch: () => void;
   setSearchQuery: (query: string) => void;
   resetSearch: () => void;
}

export const useSearchStore = create<SearchState>((set) => ({
   isSearchOpen: false,
   searchQuery: '',

   openSearch: () => set({ isSearchOpen: true }),
   closeSearch: () => set({ isSearchOpen: false }),
   toggleSearch: () => set((state) => ({ isSearchOpen: !state.isSearchOpen })),
   setSearchQuery: (query: string) => set({ searchQuery: query }),
   resetSearch: () => set({ isSearchOpen: false, searchQuery: '' }),
}));
