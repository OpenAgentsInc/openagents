import { auth } from '~/lib/auth' // Use the correct path alias
import type { LoaderFunctionArgs, ActionFunctionArgs } from "react-router"

export async function loader({ request }: LoaderFunctionArgs) {
  // console.log("Auth API GET request:", request.url);
  // Use the handler method as indicated in your auth object
  return auth.handler(request);
}

export async function action({ request }: ActionFunctionArgs) {
  // console.log("Auth API POST request:", request.url, "Method:", request.method);

  // For debugging, let's try to log the request body if it's a sign-up request
  if (request.url.includes("/sign-up/email")) {
    try {
      const clone = request.clone();
      const body = await clone.json();
      console.log("Sign-up request body:", body);
    } catch (e) {
      console.error("Could not parse request body:", e);
    }
  }

  return auth.handler(request);
}
