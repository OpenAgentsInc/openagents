import { Hono } from 'hono';
import type { OpenClawEnv } from './types';
import { requireServiceToken } from './auth/serviceToken';
import v1 from './routes/v1';
import { getOpenClawSandbox } from './sandbox/sandboxDo';
import { backupToR2 } from './sandbox/backup';

export { Sandbox } from '@cloudflare/sandbox';

const app = new Hono<{ Bindings: OpenClawEnv }>();

app.use('/v1/*', requireServiceToken);
app.route('/v1', v1);

export default {
  fetch: app.fetch,
  async scheduled(event: ScheduledEvent, env: OpenClawEnv, ctx: ExecutionContext) {
    const sandbox = getOpenClawSandbox(env);
    ctx.waitUntil(
      (async () => {
        try {
          await backupToR2(sandbox, env);
        } catch (error) {
          console.log('Scheduled backup failed:', error instanceof Error ? error.message : error);
        }
      })()
    );
  },
};
