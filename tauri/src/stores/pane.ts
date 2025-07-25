import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { Pane, PaneInput } from "@/types/pane";

export const PANE_MARGIN = 20;
export const DEFAULT_CHAT_WIDTH = 600;
export const DEFAULT_CHAT_HEIGHT = 600; // Fixed height to avoid positioning issues
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
            
            // Position new chat panes side-by-side first, then stack if needed
            const existingPanes = get().panes.filter(p => p.type === "chat");
            if (existingPanes.length > 0) {
              // Try to place side-by-side first
              const horizontalOffset = existingPanes.length * (width + PANE_MARGIN);
              const maxHorizontalPos = screenWidth - width - PANE_MARGIN;
              
              if (x + horizontalOffset <= maxHorizontalPos) {
                // Place side-by-side
                x += horizontalOffset;
              } else {
                // If not enough horizontal space, use minimal cascade
                const cascadeOffset = Math.min(existingPanes.length * 30, 150);
                x += cascadeOffset;
                y += cascadeOffset;
              }
            }
          }
        }

        // Ensure pane fits on screen with better bounds checking
        const maxX = Math.max(screenWidth - width - PANE_MARGIN, PANE_MARGIN);
        const maxY = Math.max(screenHeight - height - PANE_MARGIN - 100, PANE_MARGIN); // Leave more space at bottom
        
        x = Math.max(PANE_MARGIN, Math.min(x, maxX));
        y = Math.max(PANE_MARGIN, Math.min(y, maxY));

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
        const availableHeight = screenHeight - PANE_MARGIN * 2 - hotbarHeight;
        const availableWidth = screenWidth - PANE_MARGIN * 2;

        console.log("📐 Screen dimensions:", { screenWidth, screenHeight, availableWidth, availableHeight });

        // Separate panes by type for smart organization
        const settingsPane = panes.find(p => p.type === "settings");
        const metadataPane = panes.find(p => p.type === "metadata");
        const chatPanes = panes.filter(p => p.type === "chat");
        
        console.log("🏷️ Pane breakdown:", { 
          settings: settingsPane?.id, 
          metadata: metadataPane?.id, 
          chats: chatPanes.map(p => p.id) 
        });
        
        let newPanes = [...panes];
        let currentX = PANE_MARGIN;

        // Settings pane: narrow sidebar on the left
        if (settingsPane) {
          const settingsIndex = newPanes.findIndex(p => p.id === settingsPane.id);
          if (settingsIndex !== -1) {
            newPanes[settingsIndex] = {
              ...newPanes[settingsIndex],
              x: currentX,
              y: PANE_MARGIN,
              width: SETTINGS_PANEL_WIDTH,
              height: availableHeight,
            };
            currentX += SETTINGS_PANEL_WIDTH + PANE_MARGIN;
            console.log("⚙️ Positioned settings pane at", { x: PANE_MARGIN, y: PANE_MARGIN });
          }
        }

        // Calculate remaining space for other panes
        const remainingWidth = availableWidth - (settingsPane ? SETTINGS_PANEL_WIDTH + PANE_MARGIN : 0);
        const otherPanes: Pane[] = [];
        
        if (metadataPane) otherPanes.push(metadataPane);
        otherPanes.push(...chatPanes);

        console.log("📦 Other panes to organize:", otherPanes.length, "Remaining width:", remainingWidth);

        if (otherPanes.length === 0) {
          console.log("✅ No other panes to organize");
          set({ panes: newPanes });
          return;
        }

        // SIMPLE SIDE-BY-SIDE LAYOUT FOR ALL CASES
        const paneWidth = Math.floor(remainingWidth / otherPanes.length);
        console.log("📏 Calculated pane width:", paneWidth);

        otherPanes.forEach((pane, index) => {
          const paneIndex = newPanes.findIndex(p => p.id === pane.id);
          if (paneIndex !== -1) {
            const x = currentX + index * paneWidth;
            const y = PANE_MARGIN;
            
            newPanes[paneIndex] = {
              ...newPanes[paneIndex],
              x,
              y,
              width: paneWidth - (index < otherPanes.length - 1 ? PANE_MARGIN : 0),
              height: availableHeight,
            };
            
            console.log(`📍 Positioned pane ${pane.id} at`, { x, y, width: paneWidth });
          }
        });

        console.log("🎯 Setting new pane positions");
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