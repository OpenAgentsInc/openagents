// Common types shared between agents

export interface AgentObservation {
  id: string;
  content: string;
  timestamp: Date;
  source?: string;
  metadata?: Record<string, any>;
}

// Additional common types can be added here as needed