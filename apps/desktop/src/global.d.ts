export {};

declare global {
  interface Window {
    openAgentsDesktop?: {
      readonly config?: {
        readonly openAgentsBaseUrl?: string;
        readonly convexUrl?: string;
        readonly executorTickMs?: number;
      };
    };
  }
}
