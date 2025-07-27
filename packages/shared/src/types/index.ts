// Shared types
export interface User {
  id: string;
  name: string;
  email: string;
}

export interface Message {
  id: string;
  content: string;
  userId: string;
  timestamp: number;
}

// Recharts React 19 compatibility
export * from './recharts-compat';

// Add more shared types as needed