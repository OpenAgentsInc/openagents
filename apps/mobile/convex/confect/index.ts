// Export all Confect functions and schemas
export * from "./confect";
export * from "./schema";

// User functions
export * from "./users";
export * from "./users.schemas";

// Message functions
export * from "./messages";
export * from "./messages.schemas";

// Mobile sync functions
export * from "./mobile_sync";
export * from "./mobile_sync.schemas";

// HTTP API - temporarily disabled due to TypeScript errors
// export * from "./http-api";
// export { default as httpRouter } from "./http";

// Integration layer
export * from "./integration";

// Validation and security
export * from "./validation";

// Error tracking and monitoring
export * from "./error-tracking";

// Onboarding functions
export * from "./onboarding";
export * from "./onboarding.schemas";

// GitHub integration functions
export * from "./github"; 
export * from "./github.schemas";

// Device sync functions - disabled in mobile app (use main backend)