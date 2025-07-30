// Shared utilities and types for OpenAgents
export * from './types';
export * from './utils';

// Effect-TS services (Simple versions for stable TypeScript compilation)
export * from './services/SimpleStorageService';
export * from './services/SimpleAPMService';
export * from './services/SimpleAuthService';

// Dedicated Effect-TS Service Layer (Issue #1286)
export * from './types/session-service-types';
export * from './services/ClaudeSessionServiceSimple';
export * from './services/SessionResilienceServiceSimple';

// Effect-TS React hooks (Simple versions using basic Effect patterns)
export * from './hooks/useSimpleAPM';
export * from './hooks/useSimpleAuth';

// Export Confect-enhanced hooks (Phase 3)
// export * from './hooks/useConfectAuth'; // Disabled for now
export * from './hooks/useConfectAPM';

// Device presence hooks
export * from './hooks/useDevicePresence';

// Re-export Phase 1 mobile sync services (temporarily disabled while fixing Phase 2)
// export * from './services/MobileSyncService';
// export * from './hooks/useConfectMobileSync';