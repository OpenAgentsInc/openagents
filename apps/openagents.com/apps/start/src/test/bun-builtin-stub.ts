// Vitest-only stand-in for the `bun` builtin module.
//
// Server-chain modules may import `SQL` from the Bun builtin. Vitest
// transforms that chain through Vite, which cannot resolve the `bun` module
// specifier in a Node/happy-dom test environment. The Start server tests only
// exercise header/dispatch logic and never open a database connection, so
// aliasing the builtin to this inert stub keeps the import graph loadable
// without masking any behavior a test actually observes. Constructing the
// stub SQL client throws so accidental runtime use inside a test fails
// loudly.
export class SQL {
  constructor() {
    throw new Error(
      'bun builtin stub: SQL is not available in the vitest environment',
    )
  }
}

export default { SQL }
