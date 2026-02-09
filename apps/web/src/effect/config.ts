import { Context } from 'effect';

export type AppConfig = {
  readonly convexUrl: string;
  /** Prelaunch mode: homepage shows countdown, other routes disabled. */
  readonly prelaunch: boolean;
  /** When set, ?key={this} or cookie prelaunch_bypass=1 bypasses prelaunch. Server-only (not in client bundle). */
  readonly prelaunchBypassKey: string | null;
};

export class AppConfigService extends Context.Tag('@openagents/web/AppConfig')<
  AppConfigService,
  AppConfig
>() {}

const parsePrelaunch = (v: string | undefined): boolean =>
  v === '1' || v === 'true' || v === 'yes';

export const getAppConfig = (): AppConfig => {
  const convexUrl = (import.meta as any).env.VITE_CONVEX_URL as string | undefined;
  if (!convexUrl) {
    throw new Error('missing VITE_CONVEX_URL env var');
  }
  const prelaunch = parsePrelaunch((import.meta as any).env.VITE_PRELAUNCH as string | undefined);
  return { convexUrl, prelaunch, prelaunchBypassKey: null };
};
