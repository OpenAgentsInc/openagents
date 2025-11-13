declare module "assistant-cloud" {
  // Minimal ambient module to satisfy type-checks for optional cloud adapters.
  // Consumers should provide real types via dev dependency if needed.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  export type AssistantCloud = any;
  const mod: any;
  export default mod;
  export { AssistantCloud };
}
