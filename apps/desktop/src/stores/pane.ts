import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { Pane, PaneInput } from "@/types/pane";

// Constants matching commander project
export const PANE_MARGIN = 20;
export const CASCADE_OFFSET = 45;
export const DEFAULT_CHAT_WIDTH = 600; // Larger for better usability
export const DEFAULT_CHAT_HEIGHT = 450; // Larger for better usability
export const METADATA_PANEL_WIDTH = 320;
export const SETTINGS_PANEL_WIDTH = 320;
export const STATS_PANEL_WIDTH = 480; // Wider for charts and tables

interface ClosedPanePosition {
  x: number;
  y: number;
  width: number;
  height: number;
  content?: any;
  shouldRestore?: boolean;
}

interface Message {
  id: string;
  message_type: string;
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

interface PaneState {
  panes: Pane[];
  activePaneId: string | null;
  lastPanePosition: { x: number; y: number; width: number; height: number } | null;
  closedPanePositions: Record<string, ClosedPanePosition>;
  sessionMessages: Record<string, Message[]>;
}

interface PaneStore extends PaneState {
  // Core actions
  addPane: (pane: PaneInput) => string;
  removePane: (id: string) => void;
  updatePanePosition: (id: string, x: number, y: number) => void;
  updatePaneSize: (id: string, width: number, height: number) => void;
  bringPaneToFront: (id: string) => void;
  setActivePane: (id: string | null) => void;
  
  // Specific pane toggles (commander-style)
  openChatPane: (sessionId: string, projectPath: string) => void;
  toggleMetadataPane: () => void;
  toggleSettingsPane: () => void;
  toggleStatsPane: () => void;
  organizePanes: () => void;
  
  // Utilities
  resetPanes: () => void;
  getPaneById: (id: string) => Pane | undefined;
  updateSessionMessages: (sessionId: string, messages: Message[]) => void;
  getSessionMessages: (sessionId: string) => Message[];
}

// Position calculation utilities (exact commander implementation)
const calculateNewPanePosition = (
  _existingPanes: Pane[],
  lastPanePosition: { x: number; y: number; width: number; height: number } | null,
) => {
  const screenWidth = typeof window !== "undefined" ? window.innerWidth : 1920;
  const screenHeight = typeof window !== "undefined" ? window.innerHeight : 1080;

  if (lastPanePosition) {
    let newX = lastPanePosition.x + CASCADE_OFFSET;
    let newY = lastPanePosition.y + CASCADE_OFFSET;

    // Commander's exact wrapping logic - reset to PANE_MARGIN * 2
    if (newX + DEFAULT_CHAT_WIDTH > screenWidth - PANE_MARGIN) {
      newX = PANE_MARGIN * 2;
    }
    if (newY + DEFAULT_CHAT_HEIGHT > screenHeight - PANE_MARGIN) {
      newY = PANE_MARGIN * 2;
    }
    
    return {
      x: newX,
      y: newY,
      width: DEFAULT_CHAT_WIDTH,
      height: DEFAULT_CHAT_HEIGHT,
    };
  }

  // Fallback to top-left margin
  return {
    x: PANE_MARGIN,
    y: PANE_MARGIN,
    width: DEFAULT_CHAT_WIDTH,
    height: DEFAULT_CHAT_HEIGHT,
  };
};

// Ensure pane is visible on screen (Commander's EXACT implementation)
const ensurePaneIsVisible = (pane: Pane): Pane => {
  const screenWidth = typeof window !== "undefined" ? window.innerWidth : 1920;
  const screenHeight = typeof window !== "undefined" ? window.innerHeight : 1080;

  let { x, y, width, height } = pane;

  width = Math.max(width, 200);
  height = Math.max(height, 100);

  if (x + width > screenWidth - PANE_MARGIN) {
    x = screenWidth - width - PANE_MARGIN;
  }
  if (y + height > screenHeight - PANE_MARGIN) {
    y = screenHeight - height - PANE_MARGIN;
  }

  x = Math.max(x, PANE_MARGIN);
  y = Math.max(y, PANE_MARGIN);

  width = Math.min(width, screenWidth - x - PANE_MARGIN);
  height = Math.min(height, screenHeight - y - PANE_MARGIN);

  return { ...pane, x, y, width, height };
};

// Toggle pattern action (like commander)
const togglePaneAction = (
  set: any,
  get: any,
  options: {
    paneId: string;
    createPaneInput: (screenWidth: number, screenHeight: number, storedPosition?: ClosedPanePosition) => PaneInput;
  }
) => {
  const { panes, closedPanePositions } = get();
  const existingPane = panes.find((p: Pane) => p.id === options.paneId);
  
  if (existingPane) {
    // Close the pane and store its position
    set((state: PaneState) => ({
      panes: state.panes.filter(p => p.id !== options.paneId),
      activePaneId: state.activePaneId === options.paneId ? null : state.activePaneId,
      closedPanePositions: {
        ...state.closedPanePositions,
        [options.paneId]: {
          x: existingPane.x,
          y: existingPane.y,
          width: existingPane.width,
          height: existingPane.height,
          content: existingPane.content,
          shouldRestore: true
        }
      }
    }));
  } else {
    // Open the pane, restoring position if available
    const screenWidth = window.innerWidth;
    const screenHeight = window.innerHeight;
    const storedPosition = closedPanePositions[options.paneId];
    
    let paneInput = options.createPaneInput(screenWidth, screenHeight, storedPosition);
    
    // If we have a stored position, restore it but ensure it's still visible
    if (storedPosition && storedPosition.shouldRestore) {
      paneInput = {
        ...paneInput,
        x: Math.max(PANE_MARGIN, Math.min(storedPosition.x, screenWidth - 100)),
        y: Math.max(PANE_MARGIN, Math.min(storedPosition.y, screenHeight - 100)),
        width: storedPosition.width,
        height: storedPosition.height,
        content: storedPosition.content || paneInput.content
      };
    }
    
    const newPane = ensurePaneIsVisible({
      ...paneInput,
      id: options.paneId,
      isActive: true,
    } as Pane);

    set((state: PaneState) => ({
      panes: [...state.panes, newPane],
      activePaneId: options.paneId,
      lastPanePosition: { x: newPane.x, y: newPane.y, width: newPane.width, height: newPane.height },
    }));
  }
};

const getInitialPanes = (): Pane[] => {
  // Start with metadata panel visible
  return [{
    id: "metadata",
    type: "metadata",
    title: "OpenAgents",
    x: PANE_MARGIN,
    y: PANE_MARGIN,
    width: METADATA_PANEL_WIDTH,
    height: window.innerHeight - (PANE_MARGIN * 2) - 60, // Account for hotbar
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
      sessionMessages: {},

      addPane: (paneInput: PaneInput) => {
        const id = paneInput.id || `pane-${Date.now()}`;
        
        // Commander's exact logic: check if pane already exists first
        const existingPane = get().panes.find(p => p.id === id);
        if (existingPane) {
          // Just activate existing pane and update lastPanePosition
          set((state) => ({
            panes: state.panes.map(p => ({ ...p, isActive: p.id === id })),
            activePaneId: id,
            lastPanePosition: {
              x: existingPane.x,
              y: existingPane.y,
              width: existingPane.width,
              height: existingPane.height,
            },
          }));
          return id;
        }

        // Calculate base position using commander's algorithm
        const { panes, lastPanePosition } = get();
        const basePosition = calculateNewPanePosition(panes, lastPanePosition);

        // Create pane with commander's exact logic
        const paneBeforeEnsure = {
          id,
          type: paneInput.type,
          title: paneInput.title || `Pane ${Date.now()}`,
          x: paneInput.x ?? basePosition.x,
          y: paneInput.y ?? basePosition.y,
          width: paneInput.width ?? basePosition.width,
          height: paneInput.height ?? basePosition.height,
          isActive: true,
          dismissable: paneInput.dismissable !== undefined ? paneInput.dismissable : true,
          content: paneInput.content,
        } as Pane;

        const newPane = ensurePaneIsVisible(paneBeforeEnsure);

        // Commander's exact state update
        set((state) => ({
          panes: [...state.panes.map(p => ({ ...p, isActive: false })), newPane],
          activePaneId: newPane.id,
          lastPanePosition: {
            x: newPane.x,
            y: newPane.y,
            width: newPane.width,
            height: newPane.height,
          },
        }));

        return id;
      },

      removePane: (id: string) => {
        const pane = get().panes.find(p => p.id === id);
        if (pane) {
          set((state) => ({
            panes: state.panes.filter(p => p.id !== id),
            activePaneId: state.activePaneId === id ? null : state.activePaneId,
            closedPanePositions: {
              ...state.closedPanePositions,
              [id]: { 
                x: pane.x, 
                y: pane.y, 
                width: pane.width, 
                height: pane.height,
                content: pane.content,
                shouldRestore: true
              }
            }
          }));
        }
      },

      updatePanePosition: (id: string, x: number, y: number) => {
        set((state) => {
          let updatedPaneRef: { x: number; y: number; width: number; height: number } | null = null;
          const newPanes = state.panes.map((pane) => {
            if (pane.id === id) {
              const updated = ensurePaneIsVisible({ ...pane, x, y });
              updatedPaneRef = { x: updated.x, y: updated.y, width: updated.width, height: updated.height };
              return updated;
            }
            return pane;
          });
          
          return {
            panes: newPanes,
            lastPanePosition: updatedPaneRef || state.lastPanePosition,
          };
        });
      },

      updatePaneSize: (id: string, width: number, height: number) => {
        set((state) => {
          const newPanes = state.panes.map((pane) => {
            if (pane.id === id) {
              return ensurePaneIsVisible({ ...pane, width, height });
            }
            return pane;
          });
          
          return { panes: newPanes };
        });
      },

      bringPaneToFront: (id: string) => {
        set((state) => {
          const paneIndex = state.panes.findIndex(p => p.id === id);
          if (paneIndex === -1) return state;

          // Early return if already correct
          if (state.activePaneId === id && paneIndex === state.panes.length - 1) {
            return state;
          }

          // Commander's exact logic: move pane to end and update isActive flags
          const paneToActivate = state.panes[paneIndex];
          const otherPanes = state.panes.filter((p) => p.id !== id);
          const updatedPanes = otherPanes.map((p) => p.isActive ? { ...p, isActive: false } : p);
          const newPanes = [...updatedPanes, { ...paneToActivate, isActive: true }];

          return {
            ...state,
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
          get().bringPaneToFront(existingPane.id);
        } else {
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
        togglePaneAction(set, get, {
          paneId: "metadata",
          createPaneInput: (_screenWidth, screenHeight, storedPosition) => ({
            id: "metadata",
            type: "metadata",
            title: "History",
            dismissable: true,
            x: storedPosition?.x || PANE_MARGIN,
            y: storedPosition?.y || PANE_MARGIN,
            width: storedPosition?.width || METADATA_PANEL_WIDTH,
            height: storedPosition?.height || (screenHeight - (PANE_MARGIN * 2) - 60),
          })
        });
      },

      toggleSettingsPane: () => {
        togglePaneAction(set, get, {
          paneId: "settings",
          createPaneInput: (_screenWidth, screenHeight, storedPosition) => {
            const defaultX = METADATA_PANEL_WIDTH + PANE_MARGIN * 2;
            return {
              id: "settings",
              type: "settings",
              title: "Settings",
              dismissable: true,
              x: storedPosition?.x || defaultX,
              y: storedPosition?.y || PANE_MARGIN,
              width: storedPosition?.width || SETTINGS_PANEL_WIDTH,
              height: storedPosition?.height || (screenHeight - (PANE_MARGIN * 2) - 60),
            };
          }
        });
      },

      toggleStatsPane: () => {
        togglePaneAction(set, get, {
          paneId: "stats",
          createPaneInput: (_screenWidth, screenHeight, storedPosition) => {
            const defaultX = METADATA_PANEL_WIDTH + SETTINGS_PANEL_WIDTH + PANE_MARGIN * 3;
            return {
              id: "stats",
              type: "stats",
              title: "APM Statistics",
              dismissable: true,
              x: storedPosition?.x || defaultX,
              y: storedPosition?.y || PANE_MARGIN,
              width: storedPosition?.width || STATS_PANEL_WIDTH,
              height: storedPosition?.height || (screenHeight - (PANE_MARGIN * 2) - 60),
            };
          }
        });
      },

      organizePanes: () => {
        const { panes } = get();
        if (panes.length === 0) return;

        const screenWidth = window.innerWidth;
        const screenHeight = window.innerHeight;
        const hotbarHeight = 60;
        const availableHeight = screenHeight - PANE_MARGIN * 2 - hotbarHeight;
        const availableWidth = screenWidth - PANE_MARGIN * 2;

        // Calculate grid dimensions
        const cols = Math.ceil(Math.sqrt(panes.length));
        const rows = Math.ceil(panes.length / cols);
        
        // Calculate pane dimensions for grid
        const gridPaneWidth = Math.floor((availableWidth - (cols - 1) * PANE_MARGIN) / cols);
        const gridPaneHeight = Math.floor((availableHeight - (rows - 1) * PANE_MARGIN) / rows);

        const newPanes = panes.map((pane, index) => {
          const col = index % cols;
          const row = Math.floor(index / cols);
          
          const x = PANE_MARGIN + col * (gridPaneWidth + PANE_MARGIN);
          const y = PANE_MARGIN + row * (gridPaneHeight + PANE_MARGIN);
          
          return ensurePaneIsVisible({
            ...pane,
            x,
            y,
            width: gridPaneWidth,
            height: gridPaneHeight,
          });
        });

        // Update lastPanePosition in store
        const lastPane = newPanes[newPanes.length - 1];
        set({ 
          panes: newPanes,
          lastPanePosition: lastPane ? {
            x: lastPane.x,
            y: lastPane.y, 
            width: lastPane.width,
            height: lastPane.height
          } : null
        });
      },

      resetPanes: () => {
        set({
          panes: getInitialPanes(),
          activePaneId: "metadata",
          lastPanePosition: null,
          closedPanePositions: {},
          sessionMessages: {},
        });
      },

      getPaneById: (id: string) => {
        return get().panes.find(p => p.id === id);
      },

      updateSessionMessages: (sessionId: string, messages: Message[]) => {
        set((state) => ({
          sessionMessages: {
            ...state.sessionMessages,
            [sessionId]: messages
          }
        }));
      },

      getSessionMessages: (sessionId: string) => {
        return get().sessionMessages[sessionId] || [];
      },
    }),
    {
      name: "openagents-pane-storage-v2", // Changed version to reset storage
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        panes: state.panes,
        lastPanePosition: state.lastPanePosition,
        activePaneId: state.activePaneId,
        closedPanePositions: state.closedPanePositions,
        sessionMessages: state.sessionMessages,
      }),
      onRehydrateStorage: () => (state) => {
        if (state) {
          // Filter out chat panes since their sessions won't exist after restart
          state.panes = state.panes.filter(pane => pane.type !== "chat");
          
          // Reset active pane ID if it was pointing to a chat pane
          if (state.activePaneId && !state.panes.find(p => p.id === state.activePaneId)) {
            state.activePaneId = state.panes.length > 0 ? state.panes[0].id : null;
          }

          // Ensure all panes are visible on current screen
          state.panes = state.panes.map(ensurePaneIsVisible);
        }
      },
    }
  )
);