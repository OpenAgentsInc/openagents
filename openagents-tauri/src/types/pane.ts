export type PaneType = 
  | "chat"           // Chat sessions with Claude
  | "metadata"       // Status/sessions panel
  | "settings"       // Future settings pane
  | "help"          // Future help pane
  | string;

export interface Pane {
  id: string;                // Unique identifier for the pane
  type: PaneType;           // Type of content the pane displays
  title: string;            // Title displayed in the pane's title bar
  x: number;                // X-coordinate of the top-left corner
  y: number;                // Y-coordinate of the top-left corner
  width: number;            // Width of the pane
  height: number;           // Height of the pane
  isActive?: boolean;       // Indicates if the pane is currently active (focused)
  dismissable?: boolean;    // If true, the pane can be closed by the user
  content?: {
    sessionId?: string;     // For chat panes, the session ID
    projectPath?: string;   // For chat panes, the project path
    [key: string]: unknown; // Allows for other content properties
  };
}

// Type for input when creating a new pane
export type PaneInput = Omit<
  Pane,
  "x" | "y" | "width" | "height" | "id" | "isActive"
> & {
  id?: string; // ID might be generated or passed
  // Optional initial position/size, can be calculated by the manager if not provided
  x?: number;
  y?: number;
  width?: number;
  height?: number;
};