# @openagentsinc/agent-surface

Root package for the UI-neutral surface projectors and surface-intent helpers.
It projects canonical turn facts into safe cards and message chains that
Desktop, web, and mobile renderers decode to equivalent facts. Packet AFS-04
adds the first real projectors.

- It must not own schemas, renderers, or providers.
- It imports its schemas from `@openagentsinc/agent-runtime-schema`. It must not
  define a second wire contract.
- Root export only. No app, Electron, React, React Native, Node file or process
  API, provider SDK, SQL driver, or cloud client import.
