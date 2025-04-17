import type { AppLoadContext, EntryContext } from "react-router";
import { ServerRouter } from "react-router";
import { isbot } from "isbot";
// Import the server module with the correct method
import { renderToString } from "react-dom/server.node";

export default async function handleRequest(
  request: Request,
  responseStatusCode: number,
  responseHeaders: Headers,
  routerContext: EntryContext,
  _loadContext: AppLoadContext
) {
  // Simplify by using renderToString which is more reliable in this context
  try {
    const html = renderToString(
      <ServerRouter context={routerContext} url={request.url} />
    );
    
    responseHeaders.set("Content-Type", "text/html");
    return new Response(
      // Add DOCTYPE and wrap in html/body if needed
      `<!DOCTYPE html>${html}`,
      {
        status: responseStatusCode,
        headers: responseHeaders,
      }
    );
  } catch (error) {
    console.error("Rendering error:", error);
    
    responseHeaders.set("Content-Type", "text/html");
    return new Response(
      "<!DOCTYPE html><html><body>Server Error</body></html>",
      {
        status: 500,
        headers: responseHeaders,
      }
    );
  }
}
