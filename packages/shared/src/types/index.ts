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

// Auth types for Confect integration
export interface AuthUser {
  id: string;
  email: string;
  name?: string;
  avatar?: string;
  githubId: string;
  githubUsername: string;
}

export interface AuthState {
  isAuthenticated: boolean;
  user: AuthUser | null;
  token: string | null;
  isLoading: boolean;
  error: string | null;
}

// APM types for session tracking
export interface APMSessionData {
  deviceId: string;
  platform: 'ios' | 'android' | 'web' | 'desktop';
  sessionStart: number;
  sessionEnd: number;
  messagesSent: number;
}

// Recharts React 19 compatibility
export * from './recharts-compat';

// Add more shared types as needed