import type { LoaderFunctionArgs, ActionFunctionArgs } from "react-router"

// We'll import auth from server module directly within the handler functions
// This keeps server-only code isolated to server functions

export async function loader({ request }: LoaderFunctionArgs) {
  // Import auth server module
  const { auth } = await import('~/lib/auth.server');
  // console.log("Auth API GET request:", request.url);
  return auth.handler(request);
}

export async function action({ request }: ActionFunctionArgs) {
  // Import auth server module
  const { auth } = await import('~/lib/auth.server');
  // console.log("Auth API POST request:", request.url, "Method:", request.method);
  return auth.handler(request);
}
