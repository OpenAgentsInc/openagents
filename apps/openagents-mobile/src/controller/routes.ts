import type { AggregateType } from "./contracts";

export type ControllerRouteParams = {
  Home: undefined;
  Thread: { aggregateType: AggregateType; aggregateId: string; label: string };
  Terminal: { aggregateId: string };
  Review: { aggregateId: string };
  Files: { aggregateId: string };
  Git: { aggregateId: string };
  Connections: undefined;
  NewTask: undefined;
  Settings: undefined;
  SarahVoice: { desktopThreadRef: string | null };
};

export const controllerLinking = {
  prefixes: ["openagents://", "https://openagents.com/mobile"],
  config: {
    screens: {
      Home: "home",
      Thread: "work/:aggregateType/:aggregateId",
      Terminal: "work/thread/:aggregateId/terminal",
      Review: "work/thread/:aggregateId/review",
      Files: "work/thread/:aggregateId/files",
      Git: "work/thread/:aggregateId/git",
      Connections: "connections",
      NewTask: "new",
      Settings: "settings",
      SarahVoice: "sarah/:desktopThreadRef?",
    },
  },
};
