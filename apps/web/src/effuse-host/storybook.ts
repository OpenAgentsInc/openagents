import { listStoryMeta } from "../storybook"

export const handleStorybookApiRequest = async (request: Request): Promise<Response | null> => {
  const url = new URL(request.url)
  if (url.pathname !== "/__storybook/api/stories") return null
  if (request.method !== "GET" && request.method !== "HEAD") return new Response("Method not allowed", { status: 405 })

  const body = JSON.stringify({ stories: listStoryMeta() })
  return new Response(request.method === "HEAD" ? null : body, {
    status: 200,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  })
}

