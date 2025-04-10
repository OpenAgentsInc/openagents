import { auth } from '@/lib/auth' // Adjust the path as necessary
import type { LoaderFunctionArgs, ActionFunctionArgs } from "react-router"

export async function loader({ request }: LoaderFunctionArgs) {
  return auth.handler(request)
}

export async function action({ request }: ActionFunctionArgs) {
  return auth.handler(request)
}
