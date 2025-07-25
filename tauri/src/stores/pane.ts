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
            
            // If there are other panes, cascade them but limit the offset
            const existingPanes = get().panes.filter(p => p.type === "chat");
            if (existingPanes.length > 0) {
              const cascadeOffset = Math.min(existingPanes.length * 30, 150); // Limit cascade offset
              x += cascadeOffset;
              y += cascadeOffset;
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
        const { panes } = get();
        const visiblePanes = panes.filter(p => p.id); // All panes are visible in our system
        
        if (visiblePanes.length === 0) return;

        const screenWidth = window.innerWidth;
        const screenHeight = window.innerHeight;
        const hotbarHeight = 60;
        const availableHeight = screenHeight - PANE_MARGIN * 2 - hotbarHeight;
        const availableWidth = screenWidth - PANE_MARGIN * 2;

        // Separate panes by type for smart organization
        const settingsPane = visiblePanes.find(p => p.type === "settings");
        const metadataPane = visiblePanes.find(p => p.type === "metadata");
        const chatPanes = visiblePanes.filter(p => p.type === "chat");
        
        let newPanes = [...panes];
        let currentX = PANE_MARGIN;

        // Settings pane: narrow sidebar on the left
        if (settingsPane) {
          const settingsIndex = newPanes.findIndex(p => p.id === settingsPane.id);
          if (settingsIndex !== -1) {
            newPanes[settingsIndex] = {
              ...settingsPane,
              x: currentX,
              y: PANE_MARGIN,
              width: SETTINGS_PANEL_WIDTH,
              height: availableHeight,
            };
            currentX += SETTINGS_PANEL_WIDTH + PANE_MARGIN;
          }
        }

        // Calculate remaining space for other panes
        const remainingWidth = availableWidth - (settingsPane ? SETTINGS_PANEL_WIDTH + PANE_MARGIN : 0);
        const otherPanes: Pane[] = [];
        
        if (metadataPane) otherPanes.push(metadataPane);
        otherPanes.push(...chatPanes);

        if (otherPanes.length === 0) {
          set({ panes: newPanes });
          return;
        }

        // Organize remaining panes based on count
        if (otherPanes.length === 1) {
          // Single pane: use most of remaining width
          const pane = otherPanes[0];
          const paneIndex = newPanes.findIndex(p => p.id === pane.id);
          if (paneIndex !== -1) {
            newPanes[paneIndex] = {
              ...newPanes[paneIndex],
              x: currentX,
              y: PANE_MARGIN,
              width: remainingWidth,
              height: availableHeight,
            };
          }
        } else if (otherPanes.length === 2) {
          // Two panes: side by side, equal width with gap
          const totalGap = PANE_MARGIN; // One gap between the two panes
          const paneWidth = Math.floor((remainingWidth - totalGap) / 2);
          
          otherPanes.forEach((pane, index) => {
            const paneIndex = newPanes.findIndex(p => p.id === pane.id);
            if (paneIndex !== -1) {
              const xPos = index === 0 
                ? currentX 
                : currentX + paneWidth + PANE_MARGIN;
              
              newPanes[paneIndex] = {
                ...newPanes[paneIndex],
                x: xPos,
                y: PANE_MARGIN,
                width: paneWidth,
                height: availableHeight,
              };
            }
          });
        } else if (otherPanes.length === 3) {
          // Three panes: first one on left (~40%), other two stacked on right
          const leftPaneWidth = Math.floor(remainingWidth * 0.4);
          const rightPaneWidth = remainingWidth - leftPaneWidth - PANE_MARGIN;
          const rightPaneHeight = Math.floor((availableHeight - PANE_MARGIN) / 2);

          otherPanes.forEach((pane, index) => {
            const paneIndex = newPanes.findIndex(p => p.id === pane.id);
            if (paneIndex !== -1) {
              if (index === 0) {
                // Left pane
                newPanes[paneIndex] = {
                  ...newPanes[paneIndex],
                  x: currentX,
                  y: PANE_MARGIN,
                  width: leftPaneWidth,
                  height: availableHeight,
                };
              } else {
                // Right panes (stacked)
                newPanes[paneIndex] = {
                  ...newPanes[paneIndex],
                  x: currentX + leftPaneWidth + PANE_MARGIN,
                  y: PANE_MARGIN + (index - 1) * (rightPaneHeight + PANE_MARGIN),
                  width: rightPaneWidth,
                  height: rightPaneHeight,
                };
              }
            }
          });
        } else {
          // 4+ panes: grid layout
          const cols = Math.ceil(Math.sqrt(otherPanes.length));
          const rows = Math.ceil(otherPanes.length / cols);
          const paneWidth = Math.floor((remainingWidth - (cols - 1) * PANE_MARGIN) / cols);
          const paneHeight = Math.floor((availableHeight - (rows - 1) * PANE_MARGIN) / rows);

          otherPanes.forEach((pane, index) => {
            const paneIndex = newPanes.findIndex(p => p.id === pane.id);
            if (paneIndex !== -1) {
              const col = index % cols;
              const row = Math.floor(index / cols);
              
              newPanes[paneIndex] = {
                ...newPanes[paneIndex],
                x: currentX + col * (paneWidth + PANE_MARGIN),
                y: PANE_MARGIN + row * (paneHeight + PANE_MARGIN),
                width: paneWidth,
                height: paneHeight,
              };
            }
          });
        }

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