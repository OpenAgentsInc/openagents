import { useState, useEffect, useCallback, useRef } from 'react';
import { Effect, Runtime, Option, STM, Ref, Schedule, Duration } from 'effect';
import { Platform } from 'react-native';
import { 
  useConfectAuth 
} from './useConfectAuth';
import { 
  PermissionService, 
  PermissionType, 
  PermissionResult,
  PermissionCheckResult 
} from '../services/PermissionService';

// Onboarding step enum
export type OnboardingStep = 
  | "welcome"
  | "permissions_explained" 
  | "github_connected"
  | "repository_selected"
  | "preferences_set"
  | "completed";

// Onboarding state
export interface OnboardingState {
  step: OnboardingStep;
  startedAt: number;
  completedAt?: number;
  completedSteps: string[];
  activeRepository?: {
    url: string;
    name: string;
    owner: string;
    isPrivate: boolean;
    defaultBranch?: string;
  };
  preferences?: {
    theme?: "light" | "dark" | "system";
    notifications?: boolean;
    autoSync?: boolean;
    defaultModel?: string;
  };
  permissions?: Record<PermissionType, PermissionResult>;
  isLoading: boolean;
  error?: string;
}

// Repository information
export interface RepositoryInfo {
  url: string;
  name: string;
  owner: string;
  isPrivate: boolean;
  defaultBranch?: string;
}

// User preferences
export interface UserPreferences {
  theme?: "light" | "dark" | "system";
  notifications?: boolean;
  autoSync?: boolean;
  defaultModel?: string;
}

interface UseConfectAuthConfig {
  convexUrl: string;
  enableRealTimeSync?: boolean;
  debugMode?: boolean;
  authUrl?: string;
  clientId?: string;
  redirectUri?: string;
  scopes?: string[];
}

interface UseConfectAuthReturn {
  user: any | null;
  token: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  login: () => Promise<void>;
  logout: () => Promise<void>;
  clearError: () => void;
  syncToBackend: () => Promise<void>;
  getUserStats: (includeDeviceBreakdown?: boolean) => Promise<any>;
  requestDesktopSession: (projectPath: string, initialMessage?: string, title?: string) => Promise<string>;
}

export interface UseConfectOnboardingConfig extends Partial<UseConfectAuthConfig> {
  autoStartOnboarding?: boolean;
  requiredPermissions?: PermissionType[];
}

export interface UseConfectOnboardingReturn extends UseConfectAuthReturn {
  // Onboarding state
  onboardingState: OnboardingState;
  isOnboardingComplete: boolean;
  
  // Onboarding actions
  startOnboarding: () => Promise<void>;
  completeOnboarding: () => Promise<void>;
  updateOnboardingStep: (step: OnboardingStep, markCompleted?: boolean) => Promise<void>;
  
  // Repository management
  setActiveRepository: (repository: RepositoryInfo) => Promise<void>;
  
  // Preferences management
  setUserPreferences: (preferences: UserPreferences) => Promise<void>;
  
  // Permission management
  checkPermissions: () => Promise<PermissionCheckResult>;
  requestPermission: (type: PermissionType, reason?: string) => Promise<PermissionResult>;
  requestAllPermissions: () => Promise<PermissionResult[]>;
  
  // Utility functions
  getPermissionExplanation: (type: PermissionType) => string;
  canSkipStep: (step: OnboardingStep) => boolean;
  getNextStep: (currentStep: OnboardingStep) => OnboardingStep | null;
}

/**
 * Enhanced React hook for onboarding flow using Confect integration.
 * 
 * Extends useConfectAuth with onboarding state management, permission handling,
 * and repository/preference configuration.
 */
export function useConfectOnboarding(config: UseConfectOnboardingConfig): UseConfectOnboardingReturn {
  // Get base auth functionality
  const authConfig: UseConfectAuthConfig = {
    convexUrl: config.convexUrl || process.env.EXPO_PUBLIC_CONVEX_URL || 'https://skilled-swan-439.convex.cloud',
    enableRealTimeSync: config.enableRealTimeSync,
    debugMode: config.debugMode,
    authUrl: config.authUrl,
    clientId: config.clientId,
    redirectUri: config.redirectUri,
    scopes: config.scopes,
  };
  const confectAuth = useConfectAuth(authConfig);
  
  // Onboarding-specific state
  const [onboardingState, setOnboardingState] = useState<OnboardingState>({
    step: "welcome",
    startedAt: Date.now(),
    completedSteps: [],
    isLoading: true,
  });
  
  // Services refs
  const permissionServiceRef = useRef<any>(null);
  const confectServicesRef = useRef<any>(null);
  const onboardingStateRef = useRef<Ref.Ref<OnboardingState> | null>(null);
  
  // Initialize services
  useEffect(() => {
    const initializeServices = async () => {
      try {
        // Initialize permission service
        const permissionServiceProgram = Effect.gen(function* () {
          const permissionService = yield* PermissionService;
          return permissionService;
        });
        
        const permissionService = await Runtime.runPromise(Runtime.defaultRuntime)(
          permissionServiceProgram.pipe(
            Effect.provide(PermissionService.Default)
          )
        );
        
        permissionServiceRef.current = permissionService;
        
        // Initialize onboarding state ref
        const stateRef = await Runtime.runPromise(Runtime.defaultRuntime)(
          Ref.make(onboardingState)
        );
        onboardingStateRef.current = stateRef;
        
        // Get confect services from auth hook
        confectServicesRef.current = (confectAuth as any).confectServices;
        
      } catch (error) {
        console.error('Failed to initialize onboarding services:', error);
        setOnboardingState(prev => ({
          ...prev,
          isLoading: false,
          error: String(error)
        }));
      }
    };
    
    if (confectAuth.isAuthenticated && confectAuth.user) {
      initializeServices();
    }
  }, [confectAuth.isAuthenticated, confectAuth.user]);
  
  // Load existing onboarding progress when authenticated
  useEffect(() => {
    const loadOnboardingProgress = async () => {
      if (!confectAuth.isAuthenticated || !confectServicesRef.current) {
        return;
      }
      
      try {
        setOnboardingState(prev => ({ ...prev, isLoading: true }));
        
        const loadProgram = Effect.gen(function* () {
          // Get existing onboarding progress from backend
          const progress = yield* confectServicesRef.current.query(
            "getOnboardingProgress", 
            {}
          );
          
          return yield* Option.match(progress, {
            onNone: () => Effect.succeed(null),
            onSome: (p) => Effect.succeed(p)
          });
        });
        
        const progress = await Runtime.runPromise(Runtime.defaultRuntime)(loadProgram);
        
        if (progress) {
          setOnboardingState({
            step: progress.step,
            startedAt: progress.startedAt,
            completedAt: progress.completedAt,
            completedSteps: progress.completedSteps,
            activeRepository: progress.activeRepository,
            preferences: progress.preferences,
            isLoading: false,
          });
        } else if (config.autoStartOnboarding) {
          // Auto-start onboarding for new users
          await startOnboardingInternal();
        } else {
          setOnboardingState(prev => ({ ...prev, isLoading: false }));
        }
        
      } catch (error) {
        console.error('Failed to load onboarding progress:', error);
        setOnboardingState(prev => ({
          ...prev,
          isLoading: false,
          error: String(error)
        }));
      }
    };
    
    loadOnboardingProgress();
  }, [confectAuth.isAuthenticated, config.autoStartOnboarding]);
  
  // Internal onboarding start function
  const startOnboardingInternal = async () => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    const startProgram = Effect.gen(function* () {
      const result = yield* confectServicesRef.current.mutation(
        "startOnboarding", 
        {}
      );
      
      return result;
    });
    
    const result = await Runtime.runPromise(Runtime.defaultRuntime)(startProgram);
    
    setOnboardingState({
      step: result.step,
      startedAt: Date.now(),
      completedSteps: [],
      isLoading: false,
    });
    
    return result;
  };
  
  // Public onboarding functions
  const startOnboarding = useCallback(async () => {
    try {
      setOnboardingState(prev => ({ ...prev, isLoading: true, error: undefined }));
      await startOnboardingInternal();
    } catch (error) {
      console.error('Failed to start onboarding:', error);
      setOnboardingState(prev => ({
        ...prev,
        isLoading: false,
        error: String(error)
      }));
      throw error;
    }
  }, []);
  
  const completeOnboarding = useCallback(async () => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    try {
      setOnboardingState(prev => ({ ...prev, isLoading: true }));
      
      const completeProgram = Effect.gen(function* () {
        return yield* confectServicesRef.current.mutation(
          "completeOnboarding", 
          {}
        );
      });
      
      await Runtime.runPromise(Runtime.defaultRuntime)(completeProgram);
      
      setOnboardingState(prev => ({
        ...prev,
        step: "completed",
        completedAt: Date.now(),
        completedSteps: [...prev.completedSteps, "completed"],
        isLoading: false,
      }));
      
    } catch (error) {
      console.error('Failed to complete onboarding:', error);
      setOnboardingState(prev => ({
        ...prev,
        isLoading: false,
        error: String(error)
      }));
      throw error;
    }
  }, []);
  
  const updateOnboardingStep = useCallback(async (
    step: OnboardingStep, 
    markCompleted: boolean = true
  ) => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    try {
      setOnboardingState(prev => ({ ...prev, isLoading: true }));
      
      const updateProgram = Effect.gen(function* () {
        return yield* confectServicesRef.current.mutation(
          "updateOnboardingStep", 
          { step, markCompleted }
        );
      });
      
      await Runtime.runPromise(Runtime.defaultRuntime)(updateProgram);
      
      setOnboardingState(prev => ({
        ...prev,
        step,
        completedSteps: markCompleted 
          ? [...prev.completedSteps, prev.step]
          : prev.completedSteps,
        isLoading: false,
      }));
      
    } catch (error) {
      console.error('Failed to update onboarding step:', error);
      setOnboardingState(prev => ({
        ...prev,
        isLoading: false,
        error: String(error)
      }));
      throw error;
    }
  }, []);
  
  const setActiveRepository = useCallback(async (repository: RepositoryInfo) => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    try {
      setOnboardingState(prev => ({ ...prev, isLoading: true }));
      
      const setRepoProgram = Effect.gen(function* () {
        return yield* confectServicesRef.current.mutation(
          "setActiveRepository", 
          {
            repositoryUrl: repository.url,
            repositoryName: repository.name,
            repositoryOwner: repository.owner,
            isPrivate: repository.isPrivate,
            defaultBranch: repository.defaultBranch,
          }
        );
      });
      
      await Runtime.runPromise(Runtime.defaultRuntime)(setRepoProgram);
      
      setOnboardingState(prev => ({
        ...prev,
        activeRepository: repository,
        step: "repository_selected",
        completedSteps: [...prev.completedSteps, "repository_selected"],
        isLoading: false,
      }));
      
    } catch (error) {
      console.error('Failed to set active repository:', error);
      setOnboardingState(prev => ({
        ...prev,
        isLoading: false,
        error: String(error)
      }));
      throw error;
    }
  }, []);
  
  const setUserPreferences = useCallback(async (preferences: UserPreferences) => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    try {
      setOnboardingState(prev => ({ ...prev, isLoading: true }));
      
      const setPreferencesProgram = Effect.gen(function* () {
        return yield* confectServicesRef.current.mutation(
          "setUserPreferences", 
          { preferences }
        );
      });
      
      await Runtime.runPromise(Runtime.defaultRuntime)(setPreferencesProgram);
      
      setOnboardingState(prev => ({
        ...prev,
        preferences,
        step: "preferences_set",
        completedSteps: [...prev.completedSteps, "preferences_set"],
        isLoading: false,
      }));
      
    } catch (error) {
      console.error('Failed to set user preferences:', error);
      setOnboardingState(prev => ({
        ...prev,
        isLoading: false,
        error: String(error)
      }));
      throw error;
    }
  }, []);
  
  // Permission functions
  const checkPermissions = useCallback(async (): Promise<PermissionCheckResult> => {
    if (!permissionServiceRef.current) {
      throw new Error('Permission service not initialized');
    }
    
    const checkProgram = Effect.gen(function* () {
      const requiredPermissions = config.requiredPermissions || ["notifications", "storage", "network"];
      return yield* permissionServiceRef.current.checkRequiredPermissions(requiredPermissions);
    });
    
    const result = await Runtime.runPromise(Runtime.defaultRuntime)(checkProgram);
    
    setOnboardingState(prev => ({
      ...prev,
      permissions: result.permissions
    }));
    
    return result;
  }, [config.requiredPermissions]);
  
  const requestPermission = useCallback(async (
    type: PermissionType, 
    reason?: string
  ): Promise<PermissionResult> => {
    if (!permissionServiceRef.current || !confectServicesRef.current) {
      throw new Error('Services not initialized');
    }
    
    const requestProgram = Effect.gen(function* () {
      // Request from platform
      const result = yield* permissionServiceRef.current.requestPermission(type, reason);
      
      // Log to backend
      yield* confectServicesRef.current.mutation("requestPermission", {
        permissionType: type,
        reason,
        platform: Platform.OS,
      });
      
      // Update status in backend
      yield* confectServicesRef.current.mutation("updatePermissionStatus", {
        permissionType: type,
        status: result.status,
        platform: Platform.OS,
        fallbackEnabled: result.fallbackAvailable,
      });
      
      return result;
    });
    
    const result = await Runtime.runPromise(Runtime.defaultRuntime)(requestProgram);
    
    setOnboardingState(prev => ({
      ...prev,
      permissions: {
        ...prev.permissions,
        [type]: result
      }
    }));
    
    return result;
  }, []);
  
  const requestAllPermissions = useCallback(async (): Promise<PermissionResult[]> => {
    const requiredPermissions = config.requiredPermissions || ["notifications", "storage", "network"];
    const results: PermissionResult[] = [];
    
    for (const permissionType of requiredPermissions) {
      try {
        const result = await requestPermission(permissionType);
        results.push(result);
      } catch (error) {
        console.error(`Failed to request ${permissionType} permission:`, error);
        results.push({
          type: permissionType,
          status: "denied",
          canRetry: false,
          fallbackAvailable: true,
          reason: String(error)
        });
      }
    }
    
    return results;
  }, [config.requiredPermissions, requestPermission]);
  
  // Utility functions
  const getPermissionExplanation = useCallback((type: PermissionType): string => {
    if (!permissionServiceRef.current) {
      return `Permission for ${type} is needed for the app to function properly.`;
    }
    
    return permissionServiceRef.current.getPermissionExplanation(type);
  }, []);
  
  const canSkipStep = useCallback((step: OnboardingStep): boolean => {
    switch (step) {
      case "welcome":
      case "github_connected":
        return false; // Required steps
      case "permissions_explained":
      case "repository_selected":
      case "preferences_set":
        return true; // Optional steps
      case "completed":
        return false;
      default:
        return true;
    }
  }, []);
  
  const getNextStep = useCallback((currentStep: OnboardingStep): OnboardingStep | null => {
    const stepOrder: OnboardingStep[] = [
      "welcome",
      "permissions_explained",
      "github_connected",
      "repository_selected",
      "preferences_set",
      "completed"
    ];
    
    const currentIndex = stepOrder.indexOf(currentStep);
    if (currentIndex === -1 || currentIndex === stepOrder.length - 1) {
      return null;
    }
    
    return stepOrder[currentIndex + 1];
  }, []);
  
  // Computed properties
  const isOnboardingComplete = onboardingState.step === "completed";
  
  return {
    // Include all auth functionality
    ...confectAuth,
    
    // Onboarding state
    onboardingState,
    isOnboardingComplete,
    
    // Onboarding actions
    startOnboarding,
    completeOnboarding,
    updateOnboardingStep,
    
    // Repository management
    setActiveRepository,
    
    // Preferences management
    setUserPreferences,
    
    // Permission management
    checkPermissions,
    requestPermission,
    requestAllPermissions,
    
    // Utility functions
    getPermissionExplanation,
    canSkipStep,
    getNextStep,
  };
}