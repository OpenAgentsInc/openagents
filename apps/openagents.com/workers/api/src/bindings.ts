import { D1, R2 } from 'effect-cf'

export class OpenAgentsDatabase extends D1.Service<OpenAgentsDatabase>()(
  '@openagentsinc/OpenAgentsDatabase',
  {
    binding: 'OPENAGENTS_DB',
  },
) {}

export class ThreadFileArtifacts extends R2.Tag<ThreadFileArtifacts>()(
  '@openagentsinc/ThreadFileArtifacts',
) {}
