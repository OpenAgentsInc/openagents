import { auth } from '~/lib/auth'
import type { LoaderFunctionArgs, ActionFunctionArgs } from "react-router"
import { toRemixHandler } from "better-auth/remix" // Use the Remix helper

export async function loader({ request }: LoaderFunctionArgs) {
  return toRemixHandler(auth)(request)
}

export async function action({ request }: ActionFunctionArgs) {
  return toRemixHandler(auth)(request)
}
