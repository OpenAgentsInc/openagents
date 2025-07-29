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

// Auth types for Confect integration are now exported from services
// export interface AuthUser { ... } - moved to SimpleAuthService.ts

// export interface AuthState { ... } - moved to SimpleAuthService.ts

// APM types for session tracking are now exported from services
// export interface APMSessionData { ... } - moved to SimpleAPMService.ts

// Recharts React 19 compatibility
export * from './recharts-compat';

// Add more shared types as needed