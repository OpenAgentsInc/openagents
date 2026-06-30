# `@openagentsinc/composer-state`

Effect Schema contracts and pure reducers for the OpenAgents command composer
accepted in ADR-0013.

The package intentionally owns only serializable composer state:

- draft document blocks;
- attachment metadata;
- selections;
- typed editing steps;
- transactions and history;
- input-rule and keymap helpers;
- Markdown parse/serialize helpers for the v1 source-first subset.

It does not render DOM, own uploads, or make routing decisions from user text.
UI packages and app integrations consume this state layer and keep platform
editing behavior native.
