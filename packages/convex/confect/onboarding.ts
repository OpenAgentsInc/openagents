import { Effect, Option } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  mutation,
  query,
} from "./confect";
import {
  StartOnboardingArgs,
  StartOnboardingResult,
  GetOnboardingProgressArgs,
  GetOnboardingProgressResult,
  UpdateOnboardingStepArgs,
  UpdateOnboardingStepResult,
  SetActiveRepositoryArgs,
  SetActiveRepositoryResult,
  SetUserPreferencesArgs,
  SetUserPreferencesResult,
  CompleteOnboardingArgs,
  CompleteOnboardingResult,
  RequestPermissionArgs,
  RequestPermissionResult,
  UpdatePermissionStatusArgs,
  UpdatePermissionStatusResult,
  GetUserPermissionsArgs,
  GetUserPermissionsResult,
} from "./onboarding.schemas";

// Start onboarding process for a user
export const startOnboarding = mutation({
  args: StartOnboardingArgs,
  returns: StartOnboardingResult,
  handler: () =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Ensure user is authenticated
      const identity = yield* Effect.promise(() => auth.getUserIdentity());
      if (!identity) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      // Get the user record
      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new Error("User not found"));
      }

      const userId = user.value._id;

      // Check if onboarding already exists
      const existingProgress = yield* db
        .query("onboardingProgress")
        .withIndex("by_user_id", (q) => q.eq("userId", userId))
        .first();

      return yield* Option.match(existingProgress, {
        onSome: (progress) => Effect.succeed({
          progressId: progress._id,
          step: progress.step,
          isNewOnboarding: false
        }),
        onNone: () =>
          db.insert("onboardingProgress", {
            userId,
            step: "welcome",
            startedAt: Date.now(),
            completedSteps: [],
          }).pipe(
            Effect.map(progressId => ({
              progressId,
              step: "welcome" as const,
              isNewOnboarding: true
            }))
          )
      });
    }),
});

// Get current onboarding progress for the authenticated user
export const getOnboardingProgress = query({
  args: GetOnboardingProgressArgs,
  returns: GetOnboardingProgressResult,
  handler: () =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectQueryCtx;

      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        return Option.none();
      }

      // Get the user record
      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return Option.none();
      }

      const userId = user.value._id;

      // Get onboarding progress
      const progress = yield* db
        .query("onboardingProgress")
        .withIndex("by_user_id", (q) => q.eq("userId", userId))
        .first();

      return progress;
    }),
});

// Update onboarding step
export const updateOnboardingStep = mutation({
  args: UpdateOnboardingStepArgs,
  returns: UpdateOnboardingStepResult,
  handler: ({ step, markCompleted = false }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new Error("User not found"));
      }

      const userId = user.value._id;

      const progress = yield* db
        .query("onboardingProgress")
        .withIndex("by_user_id", (q) => q.eq("userId", userId))
        .first();

      if (Option.isNone(progress)) {
        return yield* Effect.fail(new Error("Onboarding not started"));
      }

      const currentProgress = progress.value;
      const newCompletedSteps = markCompleted 
        ? [...currentProgress.completedSteps, currentProgress.step]
        : currentProgress.completedSteps;

      yield* db.patch(currentProgress._id, {
        step,
        completedSteps: newCompletedSteps,
      });
      return currentProgress._id;
    }),
});


// Set user preferences during onboarding
export const setUserPreferences = mutation({
  args: SetUserPreferencesArgs,
  returns: SetUserPreferencesResult,
  handler: ({ preferences }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new Error("User not found"));
      }

      const userId = user.value._id;

      const progress = yield* db
        .query("onboardingProgress")
        .withIndex("by_user_id", (q) => q.eq("userId", userId))
        .first();

      if (Option.isNone(progress)) {
        return yield* Effect.fail(new Error("Onboarding not started"));
      }

      yield* db.patch(progress.value._id, {
        preferences,
        step: "preferences_set",
        completedSteps: [...progress.value.completedSteps, "preferences_set"],
      });
      return progress.value._id;
    }),
});

// Complete onboarding process
export const completeOnboarding = mutation({
  args: CompleteOnboardingArgs,
  returns: CompleteOnboardingResult,
  handler: () =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new Error("User not found"));
      }

      const userId = user.value._id;

      const progress = yield* db
        .query("onboardingProgress")
        .withIndex("by_user_id", (q) => q.eq("userId", userId))
        .first();

      if (Option.isNone(progress)) {
        return yield* Effect.fail(new Error("Onboarding not started"));
      }

      yield* db.patch(progress.value._id, {
        step: "completed",
        completedAt: Date.now(),
        completedSteps: [...progress.value.completedSteps, "completed"],
      });
      return progress.value._id;
    }),
});

// Request a specific permission
export const requestPermission = mutation({
  args: RequestPermissionArgs,
  returns: RequestPermissionResult,
  handler: ({ permissionType, reason, platform }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new Error("User not found"));
      }

      const userId = user.value._id;

      // Check if permission record already exists
      const existingPermission = yield* db
        .query("userPermissions")
        .withIndex("by_user_permission", (q) => 
          q.eq("userId", userId).eq("permissionType", permissionType)
        )
        .filter((q) => q.eq(q.field("platform"), platform))
        .first();

      return yield* Option.match(existingPermission, {
        onSome: (permission) =>
          // Update existing permission request
          db.patch(permission._id, {
            status: "not_requested",
            requestedAt: Date.now(),
            metadata: {
              reason,
              retryCount: (permission.metadata?.retryCount ?? 0) + 1,
              lastRetryAt: Date.now(),
            },
          }),
        onNone: () =>
          // Create new permission record
          db.insert("userPermissions", {
            userId,
            permissionType,
            status: "not_requested",
            requestedAt: Date.now(),
            platform,
            metadata: {
              reason,
              retryCount: 1,
              lastRetryAt: Date.now(),
            },
          })
      });
    }),
});

// Update permission status after platform response
export const updatePermissionStatus = mutation({
  args: UpdatePermissionStatusArgs,
  returns: UpdatePermissionStatusResult,
  handler: ({ permissionType, status, platform, fallbackEnabled }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new Error("User not found"));
      }

      const userId = user.value._id;

      const permission = yield* db
        .query("userPermissions")
        .withIndex("by_user_permission", (q) => 
          q.eq("userId", userId).eq("permissionType", permissionType)
        )
        .filter((q) => q.eq(q.field("platform"), platform))
        .first();

      if (Option.isNone(permission)) {
        return yield* Effect.fail(new Error("Permission request not found"));
      }

      const now = Date.now();
      const updateData: any = {
        status,
        metadata: {
          ...permission.value.metadata,
          fallbackEnabled,
        },
      };

      if (status === "granted") {
        updateData.grantedAt = now;
      } else if (status === "denied") {
        updateData.deniedAt = now;
      }

      return yield* db.patch(permission.value._id, updateData);
    }),
});

// Get all permissions for the authenticated user
export const getUserPermissions = query({
  args: GetUserPermissionsArgs,
  returns: GetUserPermissionsResult,
  handler: ({ platform }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectQueryCtx;

      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        return [];
      }

      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        return [];
      }

      const userId = user.value._id;

      let permissionsQuery = db
        .query("userPermissions")
        .withIndex("by_user_id", (q) => q.eq("userId", userId));

      if (platform) {
        permissionsQuery = permissionsQuery.filter((q) => 
          q.eq(q.field("platform"), platform)
        );
      }

      const permissions = yield* permissionsQuery.collect();
      return permissions;
    }),
});

// Set active repository for the authenticated user during onboarding
export const setActiveRepository = mutation({
  args: SetActiveRepositoryArgs,
  returns: SetActiveRepositoryResult,
  handler: ({ repositoryUrl, repositoryName, repositoryOwner, isPrivate, defaultBranch }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;
      const timestamp = new Date().toISOString();

      // Ensure user is authenticated
      const identity = (yield* Effect.promise(() => auth.getUserIdentity())) as any;
      if (!identity || !identity.subject) {
        console.error(`❌ [ONBOARDING] ${timestamp} Not authenticated`);
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      // Get the user record
      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", (identity as any).subject))
        .first();

      if (Option.isNone(user)) {
        console.error(`❌ [ONBOARDING] ${timestamp} User not found`);
        return yield* Effect.fail(new Error("User not found"));
      }

      const userId = user.value._id;

      // Get current onboarding progress
      const onboardingProgress = yield* db
        .query("onboardingProgress")
        .withIndex("by_user_id", (q) => q.eq("userId", userId))
        .first();

      if (Option.isNone(onboardingProgress)) {
        console.error(`❌ [ONBOARDING] ${timestamp} No onboarding progress found for user`);
        return yield* Effect.fail(new Error("No onboarding progress found"));
      }

      const progress = onboardingProgress.value;

      // Create repository object
      const repository = {
        url: repositoryUrl,
        name: repositoryName,
        owner: repositoryOwner,
        isPrivate,
        defaultBranch: defaultBranch || "main",
      };

      console.log(`🔄 [ONBOARDING] ${timestamp} Setting active repository for user`, {
        userId,
        repository: `${repositoryOwner}/${repositoryName}`,
        isPrivate,
        currentStep: progress.step,
      });

      // Update onboarding progress with active repository
      const updatedCompletedSteps = progress.completedSteps.includes("repository_selected")
        ? progress.completedSteps
        : [...progress.completedSteps, "repository_selected"];

      const updateData = {
        activeRepository: repository,
        completedSteps: updatedCompletedSteps,
        step: "repository_selected" as const,
      };

      // If this is the first time setting a repository, advance to next step
      if (progress.step === "github_connected") {
        console.log(`📈 [ONBOARDING] ${timestamp} Advancing to repository_selected step`);
      }

      yield* db.patch(progress._id, updateData);

      console.log(`✅ [ONBOARDING] ${timestamp} Successfully set active repository`, {
        progressId: progress._id,
        repository: `${repositoryOwner}/${repositoryName}`,
        newStep: "repository_selected",
        completedSteps: updatedCompletedSteps.length,
      });

      return progress._id;
    }),
});