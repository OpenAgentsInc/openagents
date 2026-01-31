import { useQuery } from "convex/react";
import { api } from "../../convex/_generated/api";
import { withConvexProvider } from "../lib/convex";
import { authClient } from "../lib/auth-client";

function AuthUIInner() {
  const user = useQuery(api.auth.getCurrentUser);

  const handleSignOut = async () => {
    await authClient.signOut();
    window.location.reload();
  };

  if (user === undefined) {
    return (
      <span className="text-sm text-gray-500">Loading…</span>
    );
  }

  if (user === null) {
    return (
      <div className="flex items-center gap-2">
        <a href="/login" className="text-sm text-indigo-600 hover:text-indigo-800">
          Sign in
        </a>
        <span className="text-gray-400">|</span>
        <button
          type="button"
          onClick={() =>
            authClient.signIn.social({
              provider: "github",
              callbackURL: "/",
            })
          }
          className="text-sm text-indigo-600 hover:text-indigo-800"
        >
          Sign in with GitHub
        </button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-gray-700">{user.name ?? user.email ?? "Signed in"}</span>
      <button
        type="button"
        onClick={handleSignOut}
        className="text-sm text-indigo-600 hover:text-indigo-800"
      >
        Sign out
      </button>
    </div>
  );
}

export default withConvexProvider(AuthUIInner);
