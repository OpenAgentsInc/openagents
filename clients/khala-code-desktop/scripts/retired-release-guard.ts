// This package is retained as migration and parity evidence only. Keep the
// former release implementation readable, but make package-level release
// entry points fail closed so a legacy feature flag or remembered command
// cannot publish a new Khala Code artifact.
console.error(
  'Khala Code Desktop was retired on 2026-07-09 and has no release lane. ' +
    'Build the greenfield Electron app under apps/openagents-desktop (#8574).',
)
process.exit(78)
