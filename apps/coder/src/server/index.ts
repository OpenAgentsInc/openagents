// apps/coder/src/server/index.ts
import serverApp from './server';
import handleFetch from './fetch';
import type { FetchReturnValue } from './fetch'; // Export type if needed elsewhere

export { serverApp, handleFetch, FetchReturnValue };