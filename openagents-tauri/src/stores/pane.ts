import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { Pane, PaneInput } from "@/types/pane";

export const PANE_MARGIN = 20;
export const DEFAULT_CHAT_WIDTH = 600;
export const DEFAULT_CHAT_HEIGHT = 700;
export const METADATA_PANEL_WIDTH = 320;

interface ClosedPanePosition {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface PaneState {
  panes: Pane[];
  activePaneId: string | null;
  lastPanePosition: { x: number; y: number; width: number; height: number } | null;
  closedPanePositions: Record<string, ClosedPanePosition>;
}

interface PaneStore extends PaneState {
  addPane: (pane: PaneInput) => string;
  removePane: (id: string) => void;
  updatePanePosition: (id: string, x: number, y: number) => void;
  updatePaneSize: (id: string, width: number, height: number) => void;
  bringPaneToFront: (id: string) => void;
  setActivePane: (id: string | null) => void;
  openChatPane: (sessionId: string, projectPath: string) => void;
  toggleMetadataPane: () => void;
  resetPanes: () => void;
  getPaneById: (id: string) => Pane | undefined;
}

const getInitialPanes = (): Pane[] => {
  // Start with metadata panel visible
  return [{
    id: "metadata",
    type: "metadata",
    title: "OpenAgents",
    x: PANE_MARGIN,
    y: PANE_MARGIN,
    width: METADATA_PANEL_WIDTH,
    height: window.innerHeight - (PANE_MARGIN * 4) - 60, // Account for hotbar
    dismissable: true,
    content: {}
  }];
};

export const usePaneStore = create<PaneStore>()(
  persist(
    (set, get) => ({
      panes: getInitialPanes(),
      activePaneId: "metadata",
      lastPanePosition: null,
      closedPanePositions: {},

      addPane: (paneInput: PaneInput) => {
        const screenWidth = window.innerWidth;
        const screenHeight = window.innerHeight;
        
        // Generate ID if not provided
        const id = paneInput.id || `pane-${Date.now()}`;
        
        // Calculate position if not provided
        let x = paneInput.x;
        let y = paneInput.y;
        let width = paneInput.width || DEFAULT_CHAT_WIDTH;
        let height = paneInput.height || DEFAULT_CHAT_HEIGHT;

        if (x === undefined || y === undefined) {
          // Try to use stored position for this pane
          const storedPosition = get().closedPanePositions[id];
          if (storedPosition) {
            ({ x, y, width, height } = storedPosition);
          } else {
            // Smart positioning: place to the right of metadata panel
            x = METADATA_PANEL_WIDTH + PANE_MARGIN * 2;
            y = PANE_MARGIN;
            
            // If there are other panes, cascade them
            const existingPanes = get().panes.filter(p => p.type === "chat");
            if (existingPanes.length > 0) {
              x += existingPanes.length * 30;
              y += existingPanes.length * 30;
            }
          }
        }

        // Ensure pane fits on screen
        x = Math.max(PANE_MARGIN, Math.min(x, screenWidth - width - PANE_MARGIN));
        y = Math.max(PANE_MARGIN, Math.min(y, screenHeight - height - PANE_MARGIN - 60));

        const newPane: Pane = {
          ...paneInput,
          id,
          x,
          y,
          width,
          height,
          isActive: true,
        };

        set((state) => ({
          panes: [...state.panes, newPane],
          activePaneId: id,
          lastPanePosition: { x, y, width, height },
        }));

        return id;
      },

      removePane: (id: string) => {
        const pane = get().panes.find(p => p.id === id);
        if (pane) {
          // Store position for restoration
          set((state) => ({
            panes: state.panes.filter(p => p.id !== id),
            activePaneId: state.activePaneId === id ? null : state.activePaneId,
            closedPanePositions: {
              ...state.closedPanePositions,
              [id]: { x: pane.x, y: pane.y, width: pane.width, height: pane.height }
            }
          }));
        }
      },

      updatePanePosition: (id: string, x: number, y: number) => {
        set((state) => ({
          panes: state.panes.map(pane =>
            pane.id === id ? { ...pane, x, y } : pane
          ),
        }));
      },

      updatePaneSize: (id: string, width: number, height: number) => {
        set((state) => ({
          panes: state.panes.map(pane =>
            pane.id === id ? { ...pane, width, height } : pane
          ),
        }));
      },

      bringPaneToFront: (id: string) => {
        set((state) => {
          const paneIndex = state.panes.findIndex(p => p.id === id);
          if (paneIndex === -1) return state;

          const newPanes = [...state.panes];
          const [pane] = newPanes.splice(paneIndex, 1);
          newPanes.push(pane);

          return {
            panes: newPanes,
            activePaneId: id,
          };
        });
      },

      setActivePane: (id: string | null) => {
        set({ activePaneId: id });
      },

      openChatPane: (sessionId: string, projectPath: string) => {
        const existingPane = get().panes.find(p => 
          p.type === "chat" && p.content?.sessionId === sessionId
        );

        if (existingPane) {
          // Just bring it to front if it already exists
          get().bringPaneToFront(existingPane.id);
        } else {
          // Create new chat pane
          get().addPane({
            id: `chat-${sessionId}`,
            type: "chat",
            title: projectPath.split('/').pop() || "Chat",
            dismissable: true,
            content: {
              sessionId,
              projectPath,
            }
          });
        }
      },

      toggleMetadataPane: () => {
        const metadataPane = get().panes.find(p => p.id === "metadata");
        
        if (metadataPane) {
          get().removePane("metadata");
        } else {
          const storedPosition = get().closedPanePositions["metadata"];
          get().addPane({
            id: "metadata",
            type: "metadata",
            title: "OpenAgents",
            dismissable: true,
            ...(storedPosition || {
              x: PANE_MARGIN,
              y: PANE_MARGIN,
              width: METADATA_PANEL_WIDTH,
              height: window.innerHeight - (PANE_MARGIN * 4) - 60,
            })
          });
        }
      },

      resetPanes: () => {
        set({
          panes: getInitialPanes(),
          activePaneId: "metadata",
          lastPanePosition: null,
          closedPanePositions: {},
        });
      },

      getPaneById: (id: string) => {
        return get().panes.find(p => p.id === id);
      },
    }),
    {
      name: "openagents-pane-storage",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        panes: state.panes,
        lastPanePosition: state.lastPanePosition,
        activePaneId: state.activePaneId,
        closedPanePositions: state.closedPanePositions,
      }),
      onRehydrateStorage: () => (state) => {
        if (state) {
          // Filter out chat panes since their sessions won't exist after restart
          state.panes = state.panes.filter(pane => pane.type !== "chat");
          
          // Reset active pane ID if it was pointing to a chat pane
          if (state.activePaneId && !state.panes.find(p => p.id === state.activePaneId)) {
            state.activePaneId = state.panes.length > 0 ? state.panes[0].id : null;
          }
        }
      },
    }
  )
);