export const SARAH_BETA_VOICE_BASE_URL =
  "https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app" as const;

export const SARAH_BETA_VOICE_ENVIRONMENT = "Staging beta" as const;

export const sarahVoiceApiHost = (): string => new URL(SARAH_BETA_VOICE_BASE_URL).hostname;
