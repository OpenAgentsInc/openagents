export type AgentWorkerEnv = {
  OA_INTERNAL_KEY: string;
  OPENAI_API_KEY?: string;
  OPENAI_BASE_URL?: string;
  OPENCLAW_API_BASE?: string;
  OPENAGENTS_API_URL?: string;
  PUBLIC_API_URL?: string;
  ThreadAgent: DurableObjectNamespace;
};

export type DurableObjectNamespace = {
  idFromName(name: string): DurableObjectId;
  get(id: DurableObjectId): DurableObjectStub;
};

export type DurableObjectId = unknown;

export type DurableObjectStub = {
  fetch(request: Request): Promise<Response>;
};
