import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { Pane, PaneInput } from "@/types/pane";

export const PANE_MARGIN = 20;
export const CASCADE_OFFSET = 45; // Same as commander project
export const DEFAULT_CHAT_WIDTH = 600;
export const DEFAULT_CHAT_HEIGHT = 400;
export const METADATA_PANEL_WIDTH = 320;
export const SETTINGS_PANEL_WIDTH = 320;

interface ClosedPanePosition {
  x: number;
  y: number;
  width: number;
  height: number;
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
  sessionMessages: Record<string, Message[]>; // Store messages by sessionId
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
  toggleSettingsPane: () => void;
  organizePanes: () => void;
  resetPanes: () => void;
  getPaneById: (id: string) => Pane | undefined;
  updateSessionMessages: (sessionId: string, messages: Message[]) => void;
  getSessionMessages: (sessionId: string) => Message[];
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
      sessionMessages: {},

      addPane: (paneInput: PaneInput) => {
        console.log("➕ addPane called with:", paneInput);
        const screenWidth = window.innerWidth;
        const screenHeight = window.innerHeight;
        
        // Generate ID if not provided
        const id = paneInput.id || `pane-${Date.now()}`;
        
        // Calculate position if not provided
        let x = paneInput.x;
        let y = paneInput.y;
        let width = paneInput.width || DEFAULT_CHAT_WIDTH;
        let height = paneInput.height || DEFAULT_CHAT_HEIGHT;

        console.log("📍 Initial position values:", { x, y, width, height });

        if (x === undefined || y === undefined) {
          // Try to use stored position for this pane
          const storedPosition = get().closedPanePositions[id];
          if (storedPosition) {
            ({ x, y, width, height } = storedPosition);
            console.log("📦 Using stored position:", { x, y, width, height });
          } else {
            // Simple cascade positioning like commander project
            const existingPanes = get().panes;
            
            // Start position
            x = PANE_MARGIN;
            y = PANE_MARGIN;
            
            // Apply cascade offset based on existing panes
            if (existingPanes.length > 0) {
              x += existingPanes.length * CASCADE_OFFSET;
              y += existingPanes.length * CASCADE_OFFSET;
              
              // Boundary wrapping
              const maxX = screenWidth - width - PANE_MARGIN;
              const maxY = screenHeight - height - PANE_MARGIN - 60; // hotbar space
              
              if (x > maxX) x = PANE_MARGIN;
              if (y > maxY) y = PANE_MARGIN;
            }
            
            console.log("🎯 Cascade position:", { x, y, existingCount: existingPanes.length });
          }
        }

        // Ensure pane fits on screen with better bounds checking
        const maxX = Math.max(screenWidth - width - PANE_MARGIN, PANE_MARGIN);
        const maxY = Math.max(screenHeight - height - PANE_MARGIN - 60, PANE_MARGIN); // Leave space for hotbar
        
        console.log("🔒 Bounds checking:", { maxX, maxY, originalX: x, originalY: y });
        
        x = Math.max(PANE_MARGIN, Math.min(x, maxX));
        y = Math.max(PANE_MARGIN, Math.min(y, maxY));

        console.log("✅ Final position after bounds check:", { x, y, width, height });

        const newPane: Pane = {
          ...paneInput,
          id,
          x,
          y,
          width,
          height,
          isActive: true,
        };

        console.log("🆕 Creating new pane:", newPane);

        set((state) => ({
          panes: [...state.panes, newPane],
          activePaneId: id,
          lastPanePosition: { x, y, width, height },
        }));

        console.log("📊 Panes after adding:", get().panes.map(p => ({ id: p.id, x: p.x, y: p.y })));

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
            title: "History",
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

      toggleSettingsPane: () => {
        const settingsPane = get().panes.find(p => p.id === "settings");
        
        if (settingsPane) {
          get().removePane("settings");
        } else {
          const storedPosition = get().closedPanePositions["settings"];
          // Position settings pane to the right of other panes
          const defaultX = METADATA_PANEL_WIDTH + PANE_MARGIN * 2;
          get().addPane({
            id: "settings",
            type: "settings",
            title: "Settings",
            dismissable: true,
            ...(storedPosition || {
              x: defaultX,
              y: PANE_MARGIN,
              width: SETTINGS_PANEL_WIDTH,
              height: window.innerHeight - (PANE_MARGIN * 4) - 60,
            })
          });
        }
      },

      organizePanes: () => {
        console.log("🔧 organizePanes called");
        const { panes } = get();
        console.log("📊 Current panes:", panes.length, panes.map(p => ({ id: p.id, type: p.type })));
        
        if (panes.length === 0) return;

        const screenWidth = window.innerWidth;
        const screenHeight = window.innerHeight;
        const hotbarHeight = 60;
        // Use cascade offset constant
        
        console.log("📐 Screen dimensions:", { screenWidth, screenHeight });

        // Start position: top-left with margin
        let currentX = PANE_MARGIN;
        let currentY = PANE_MARGIN;
        
        // Organize panes with simple cascade positioning
        const newPanes = panes.map((pane, index) => {
          // Calculate position with cascade offset
          const x = currentX + (index * CASCADE_OFFSET);
          const y = currentY + (index * CASCADE_OFFSET);
          
          // Check if pane would go off-screen
          const maxX = screenWidth - pane.width - PANE_MARGIN;
          const maxY = screenHeight - pane.height - PANE_MARGIN - hotbarHeight;
          
          // Wrap to start position if hitting boundaries
          const finalX = x > maxX ? PANE_MARGIN : x;
          const finalY = y > maxY ? PANE_MARGIN : y;
          
          console.log(`📍 Pane ${pane.id}: cascade(${x}, ${y}) -> final(${finalX}, ${finalY})`);
          
          return {
            ...pane,
            x: finalX,
            y: finalY,
          };
        });

        console.log("🎯 Setting new pane positions");
        console.log("📋 Final pane positions:", newPanes.map(p => ({ 
          id: p.id, 
          type: p.type, 
          x: p.x, 
          y: p.y 
        })));
        set({ panes: newPanes });
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
      name: "openagents-pane-storage",
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
        }
      },
    }
  )
);