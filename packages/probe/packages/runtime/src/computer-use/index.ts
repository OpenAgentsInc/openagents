// Computer-use tool surface for Khala (`openagents/khala`).
//
// Gives Khala the same developer tools a human uses — a real browser, a real
// terminal, a scoped filesystem — exposed as Probe LLM tools with an action
// timeline. The retained `node-pty` adapter imports its dependency lazily so
// importing this index does not start a terminal; unit tests inject fakes
// against the seams in `page.ts` and `terminal.ts`.

export * from "./timeline";
export * from "./page";
export * from "./browser";
export * from "./terminal";
export * from "./filesystem";
export * from "./tools";
export * from "./node-pty";
