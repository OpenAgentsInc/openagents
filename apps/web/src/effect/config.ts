import { Context } from 'effect';

export type AppConfig = {
  readonly convexUrl: string;
};

export class AppConfigService extends Context.Tag('@openagents/web/AppConfig')<
  AppConfigService,
  AppConfig
>() {}

export const getAppConfig = (): AppConfig => {
  const convexUrl = (import.meta as any).env.VITE_CONVEX_URL as string | undefined;
  if (!convexUrl) {
    throw new Error('missing VITE_CONVEX_URL env var');
  }
  return { convexUrl };
};
