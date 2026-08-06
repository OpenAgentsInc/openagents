import type { AggregateType } from "./contracts";

export type ControllerRouteParams = {
  Home: undefined;
  Inbox: undefined;
  Thread: {
    aggregateType: AggregateType;
    aggregateId: string;
    workspaceId?: string;
    label?: string;
  };
  Terminal: { aggregateId: string };
  Review: { aggregateId: string };
  Files: { aggregateId: string };
  Git: { aggregateId: string };
  Connections: undefined;
  Intake: undefined;
  NewTask: undefined;
  Settings: undefined;
  SarahVoice: { desktopThreadRef: string | null };
};

export const controllerLinking = {
  prefixes: ["openagents://", "https://openagents.com/mobile"],
  config: {
    screens: {
      Home: "home",
      Inbox: "inbox",
      Thread: "work/:aggregateType/:aggregateId",
      Terminal: "work/thread/:aggregateId/terminal",
      Review: "work/thread/:aggregateId/review",
      Files: "work/thread/:aggregateId/files",
      Git: "work/thread/:aggregateId/git",
      Connections: "connections",
      Intake: "intake",
      NewTask: "new",
      Settings: "settings",
      SarahVoice: "sarah/:desktopThreadRef?",
    },
  },
};
