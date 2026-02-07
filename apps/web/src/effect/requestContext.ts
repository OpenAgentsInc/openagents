import { Context } from 'effect';

/**
 * Request context for Effect services.
 *
 * In the TanStack Start host, we have a singleton Effect runtime. To keep
 * request-scoped concerns (cookies, auth, requestId) explicit without
 * rebuilding the runtime per request, we thread the current Request through
 * this service and override it at the edge (route handlers, SSR entry, etc.).
 *
 * On the client, this will always be `{ _tag: "Client" }`.
 */
export type RequestContext =
  | { readonly _tag: 'Server'; readonly request: Request }
  | { readonly _tag: 'Client' }
  | { readonly _tag: 'MissingServerRequest' };

export class RequestContextService extends Context.Tag('@openagents/web/RequestContext')<
  RequestContextService,
  RequestContext
>() {}

export const makeServerRequestContext = (request: Request): RequestContext => ({
  _tag: 'Server',
  request,
});

export const makeDefaultRequestContext = (): RequestContext => {
  // Vite replaces this at build-time, so SSR code can be tree-shaken correctly.
  if ((import.meta as any).env?.SSR) {
    return { _tag: 'MissingServerRequest' };
  }
  return { _tag: 'Client' };
};

