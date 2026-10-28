# Appendix F. Sources

## Coax

- [Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md): the language design and implementation
  plan, including the stages in Chapter 21.
- [Bootstrap grammar](https://github.com/OpenAgentsInc/openagents/tree/main/coax/grammar.ebnf): the subset the current compiler accepts.
- [Compiler guide](https://github.com/OpenAgentsInc/openagents/tree/main/coax/README.md): builds, commands, tests and current limits.
- [Bootstrap seeds](https://github.com/OpenAgentsInc/openagents/tree/main/coax/bootstrap/README.md): how the generated lexer and
  parser seeds are produced and refreshed, and what they do and don't prove.
- [Descriptor importer contract](https://github.com/OpenAgentsInc/openagents/tree/main/coax/proto.md): what `#import_proto` and
  `coax proto` accept and generate.
- [The compiler's lexer](https://github.com/OpenAgentsInc/openagents/tree/main/coax/lib/compiler/lexer.coax) and
  [parser](https://github.com/OpenAgentsInc/openagents/tree/main/coax/lib/compiler/parser.coax): the Coax modules Chapter 20 walks
  through.

## Coax in OpenAgents

- [OpenAgents iOS app](https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents-ios/README.md): how the app
  builds and calls the Coax files in `metareal/`.
- [Metareal](https://github.com/OpenAgentsInc/openagents/tree/main/docs/verse/metareal-engine.md): engine scope and native paths.

## Protocol

- [OpenAgents Protocol](https://github.com/OpenAgentsInc/openagents/tree/main/docs/protocol/openagents-protocol-spec.md): the
  defining contract for every OpenAgents surface, including Verse.
- [Root schemas](https://github.com/OpenAgentsInc/openagents/tree/main/proto/README.md): actual sources, namespaces, modules,
  descriptors, compatibility snapshots and executable checks.
- [Verse chapter](https://github.com/OpenAgentsInc/openagents/tree/main/docs/verse/openagents-verse-protocol.md): world and
  product protocol catalog and dependency order.
- [Recovery provenance](https://github.com/OpenAgentsInc/openagents/tree/main/proto/recovery.json): pre-reset source revision
  and exact source/snapshot/vendor hashes.
- [Metareal migration](https://github.com/OpenAgentsInc/openagents/tree/main/proto/migrations/README.md): canonical names,
  explicit old readers and versioned ID/signature transitions.
