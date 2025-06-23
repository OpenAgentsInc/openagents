import type { RouteHandler } from "../types"

interface ConvertedRoute {
  method: "GET" | "POST" | "PUT" | "DELETE" | "PATCH"
  path: string
  handler: RouteHandler
}

/**
 * Converts an Elysia router to standard routes
 * This is a temporary adapter for migration purposes
 */
export function convertElysiaRouter(router: any, prefix: string = ""): Array<ConvertedRoute> {
  const routes: Array<ConvertedRoute> = []

  if (!router || !router.routes) {
    return routes
  }

  // Process each route from the Elysia router
  for (const route of router.routes) {
    const { handler, method, path } = route

    if (!method || !path || !handler) {
      continue
    }

    const fullPath = prefix + path

    // Convert the handler to a standard RouteHandler
    const convertedHandler: RouteHandler = async (context: any) => {
      try {
        // Parse body if needed
        let body: any = undefined
        const request = context.request

        if (method === "post" || method === "put" || method === "patch") {
          const contentType = request.headers["content-type"] || ""

          if (contentType.includes("application/json")) {
            const text = await request.text()
            try {
              body = JSON.parse(text)
            } catch {
              body = text
            }
          } else if (contentType.includes("application/x-www-form-urlencoded")) {
            const formData = await request.formData()
            body = Object.fromEntries(formData)
          }
        }

        // Create Elysia-like context
        const elysiaContext = {
          body,
          query: Object.fromEntries(new URL(request.url).searchParams),
          headers: request.headers,
          request,
          params: context.params
        }

        // Call the handler
        let result
        if (typeof handler === "function") {
          result = await handler(elysiaContext)
        } else {
          result = handler
        }

        // Return the result
        return result
      } catch (error) {
        console.error("Error in Elysia adapter:", error)
        return new Response(
          JSON.stringify({ error: "Internal server error" }),
          {
            status: 500,
            headers: { "content-type": "application/json" }
          }
        )
      }
    }

    routes.push({
      method: method.toUpperCase() as any,
      path: fullPath,
      handler: convertedHandler
    })
  }

  return routes
}
