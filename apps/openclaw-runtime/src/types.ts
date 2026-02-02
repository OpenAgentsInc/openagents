import type { Sandbox } from '@cloudflare/sandbox';

export type OpenClawEnv = {
  OPENAGENTS_SERVICE_TOKEN: string;
  OPENCLAW_INSTANCE_ID?: string;
  OPENCLAW_INSTANCE_TYPE?: string;
  OPENCLAW_VERSION?: string;
  OPENCLAW_GATEWAY_TOKEN?: string;
  OPENCLAW_BIND_MODE?: string;
  OPENCLAW_DEV_MODE?: string;
  OPENCLAW_BUCKET: R2Bucket;
  Sandbox: DurableObjectNamespace<Sandbox>;
};
