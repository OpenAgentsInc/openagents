import {
  convexAuthNextjsMiddleware,
  createRouteMatcher,
  nextjsMiddlewareRedirect,
} from "@convex-dev/auth/nextjs/server";

const isSignInPage = createRouteMatcher(["/signin"]);

// Define routes that require authentication
const isProtectedRoute = createRouteMatcher([
  "/server",
  "/projects(.*)",
  "/agent(.*)",
  "/workspace(.*)",
  "/settings(.*)",
  "/dashboard(.*)",
  "/app(.*)",
]);

export default convexAuthNextjsMiddleware(async (request, { convexAuth }) => {
  // If user is already authenticated and tries to access signin page, redirect to home
  if (isSignInPage(request) && (await convexAuth.isAuthenticated())) {
    return nextjsMiddlewareRedirect(request, "/");
  }
  
  // If user is not authenticated and tries to access protected route, redirect to signin
  if (isProtectedRoute(request) && !(await convexAuth.isAuthenticated())) {
    return nextjsMiddlewareRedirect(request, "/signin");
  }
});

export const config = {
  // The following matcher runs middleware on all routes
  // except static assets.
  matcher: ["/((?!.*\\..*|_next).*)", "/", "/(api|trpc)(.*)"],
};
