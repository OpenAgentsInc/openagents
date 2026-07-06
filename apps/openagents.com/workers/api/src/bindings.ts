import type { WorkerBindings } from '@openagentsinc/sync-worker'
import { D1, R2 } from 'effect-cf'

import type { OpenAgentsWorkerConfigEnv } from './config'

export type OpenAgentsWorkerEnv = WorkerBindings & OpenAgentsWorkerConfigEnv

export class OpenAgentsDatabase extends D1.Service<OpenAgentsDatabase>()(
  '@openagentsinc/OpenAgentsDatabase',
  {
    binding: 'OPENAGENTS_DB',
  },
) {}

export class ThreadFileArtifacts extends R2.Tag<ThreadFileArtifacts>()(
  '@openagentsinc/ThreadFileArtifacts',
) {}
