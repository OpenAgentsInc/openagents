import { auth } from '~/lib/auth' // Use the correct path alias
import type { LoaderFunctionArgs, ActionFunctionArgs } from "react-router"

// Define a handler function to work around type issues
const handleRequest = (request: Request) => {
  return auth.handler(request);
};

export async function loader({ request }: LoaderFunctionArgs) {
  // console.log("Auth API GET request:", request.url);
  return handleRequest(request);
}

export async function action({ request }: ActionFunctionArgs) {
  // console.log("Auth API POST request:", request.url, "Method:", request.method);
  return handleRequest(request);
}
