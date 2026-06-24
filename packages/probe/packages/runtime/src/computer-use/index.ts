// Computer-use tool surface for Khala (`openagents/khala`).
//
// Gives Khala the same developer tools a human uses — a real browser, a real
// terminal, a scoped filesystem — exposed as Probe LLM tools with an action
// timeline. The real adapters (`playwright-page`, `node-pty`) import their heavy
// dependencies lazily/dynamically so importing this index does not pull in
// chromium; unit tests inject fakes against the seams in `page.ts` / `terminal.ts`.

export * from "./timeline";
export * from "./page";
export * from "./browser";
export * from "./terminal";
export * from "./terminal-snapshot";
export * from "./filesystem";
export * from "./tools";
export * from "./playwright-page";
export * from "./node-pty";
