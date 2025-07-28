import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";

// Onboarding step enum
export const OnboardingStep = Schema.Literal(
  "welcome",
  "permissions_explained", 
  "github_connected",
  "repository_selected",
  "preferences_set",
  "completed"
);

// Permission type enum
export const PermissionType = Schema.Literal(
  "notifications",
  "storage", 
  "network",
  "camera",
  "microphone",
  "location"
);

// Permission status enum
export const PermissionStatus = Schema.Literal(
  "granted", 
  "denied", 
  "not_requested", 
  "restricted"
);

// Repository structure
export const RepositorySchema = Schema.Struct({
  url: Schema.String.pipe(Schema.nonEmpty()),
  name: Schema.String.pipe(Schema.nonEmpty()),
  owner: Schema.String.pipe(Schema.nonEmpty()),
  isPrivate: Schema.Boolean,
  defaultBranch: Schema.optional(Schema.String),
});

// User preferences structure
export const UserPreferencesSchema = Schema.Struct({
  theme: Schema.optional(Schema.Literal("light", "dark", "system")),
  notifications: Schema.optional(Schema.Boolean),
  autoSync: Schema.optional(Schema.Boolean),
  defaultModel: Schema.optional(Schema.String),
});

// Permission metadata structure
export const PermissionMetadataSchema = Schema.Struct({
  reason: Schema.optional(Schema.String),
  fallbackEnabled: Schema.optional(Schema.Boolean),
  retryCount: Schema.optional(Schema.Number),
  lastRetryAt: Schema.optional(Schema.Number),
});

// Start Onboarding
export const StartOnboardingArgs = Schema.Struct({});

export const StartOnboardingResult = Schema.Struct({
  progressId: Id.Id("onboardingProgress"),
  step: OnboardingStep,
  isNewOnboarding: Schema.Boolean,
});

// Get Onboarding Progress
export const GetOnboardingProgressArgs = Schema.Struct({});

export const GetOnboardingProgressResult = Schema.Option(
  Schema.Struct({
    _id: Id.Id("onboardingProgress"),
    _creationTime: Schema.Number,
    userId: Id.Id("users"),
    step: OnboardingStep,
    startedAt: Schema.Number,
    completedAt: Schema.optional(Schema.Number),
    completedSteps: Schema.Array(Schema.String),
    activeRepository: Schema.optional(RepositorySchema),
    preferences: Schema.optional(UserPreferencesSchema),
    permissions: Schema.optional(
      Schema.Struct({
        notifications: Schema.Boolean,
        storage: Schema.Boolean,
        network: Schema.Boolean,
        camera: Schema.optional(Schema.Boolean),
        microphone: Schema.optional(Schema.Boolean),
      })
    ),
    metadata: Schema.optional(
      Schema.Struct({
        platform: Schema.optional(Schema.String),
        version: Schema.optional(Schema.String),
        deviceModel: Schema.optional(Schema.String),
        skipReason: Schema.optional(Schema.String),
      })
    ),
  })
);

// Update Onboarding Step
export const UpdateOnboardingStepArgs = Schema.Struct({
  step: OnboardingStep,
  markCompleted: Schema.optional(Schema.Boolean),
});

export const UpdateOnboardingStepResult = Id.Id("onboardingProgress");

// Set Active Repository
export const SetActiveRepositoryArgs = Schema.Struct({
  repositoryUrl: Schema.String.pipe(Schema.nonEmpty()),
  repositoryName: Schema.String.pipe(Schema.nonEmpty()),
  repositoryOwner: Schema.String.pipe(Schema.nonEmpty()),
  isPrivate: Schema.Boolean,
  defaultBranch: Schema.optional(Schema.String),
});

export const SetActiveRepositoryResult = Id.Id("onboardingProgress");

// Set User Preferences
export const SetUserPreferencesArgs = Schema.Struct({
  preferences: UserPreferencesSchema,
});

export const SetUserPreferencesResult = Id.Id("onboardingProgress");

// Complete Onboarding
export const CompleteOnboardingArgs = Schema.Struct({});

export const CompleteOnboardingResult = Id.Id("onboardingProgress");

// Request Permission
export const RequestPermissionArgs = Schema.Struct({
  permissionType: PermissionType,
  reason: Schema.optional(Schema.String),
  platform: Schema.String.pipe(Schema.nonEmpty()),
});

export const RequestPermissionResult = Id.Id("userPermissions");

// Update Permission Status
export const UpdatePermissionStatusArgs = Schema.Struct({
  permissionType: PermissionType,
  status: PermissionStatus,
  platform: Schema.String.pipe(Schema.nonEmpty()),
  fallbackEnabled: Schema.optional(Schema.Boolean),
});

export const UpdatePermissionStatusResult = Id.Id("userPermissions");

// Get User Permissions
export const GetUserPermissionsArgs = Schema.Struct({
  platform: Schema.optional(Schema.String),
});

export const GetUserPermissionsResult = Schema.Array(
  Schema.Struct({
    _id: Id.Id("userPermissions"),
    _creationTime: Schema.Number,
    userId: Id.Id("users"),
    permissionType: PermissionType,
    status: PermissionStatus,
    requestedAt: Schema.optional(Schema.Number),
    grantedAt: Schema.optional(Schema.Number),
    deniedAt: Schema.optional(Schema.Number),
    platform: Schema.String,
    metadata: Schema.optional(PermissionMetadataSchema),
  })
);