import { handleCallbackRoute } from "@workos/authkit-tanstack-react-start"

// Transitional: preserve WorkOS OAuth callback behavior during Phase 6 cutover.
//
// This keeps the existing `state` decoding + session header behavior working while
// the rest of the app moves off the TanStack host. We can replace this with a
// native `@workos/authkit-session` handler once React/TanStack deps are removed.
const callbackHandler = handleCallbackRoute()

export const handleCallbackRequest = (request: Request): Promise<Response> => {
  if (request.method !== "GET") {
    return Promise.resolve(new Response("Method not allowed", { status: 405 }))
  }
  // The WorkOS helper expects a TanStack-style handler signature `({ request }) => Response`.
  return (callbackHandler as any)({ request })
}
