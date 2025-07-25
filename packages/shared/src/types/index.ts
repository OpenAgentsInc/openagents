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

// Add more shared types as needed