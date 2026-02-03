import { useQuery } from "convex/react";
import { api } from "../../../convex/_generated/api";
import { withConvexProvider } from "@/lib/convex";
import { authClient } from "@/lib/auth-client";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";

export type AuthUIUserState = undefined | null | { name?: string; email?: string };

/** Presentational auth UI for a given user state. Use in Storybook without Convex. */
export function AuthUIView({ user }: { user: AuthUIUserState }) {
  const handleSignOut = async () => {
    await authClient.signOut();
    window.location.reload();
  };

  if (user === undefined) {
    return <Skeleton className="h-5 w-16" />;
  }

  if (user === null) {
    return (
      <div className="flex items-center gap-2">
        <Button variant="link" asChild size="sm">
          <a href="/login">Log in</a>
        </Button>
        <span className="text-muted-foreground">|</span>
        <Button
          type="button"
          variant="link"
          size="sm"
          onClick={() =>
            authClient.signIn.social({
              provider: "github",
              callbackURL: "/",
            })
          }
        >
          Log in with GitHub
        </Button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-muted-foreground">
        {user.name ?? user.email ?? "Logged in"}
      </span>
      <Button type="button" variant="link" size="sm" onClick={handleSignOut}>
        Sign out
      </Button>
    </div>
  );
}

function AuthUIInner() {
  const user = useQuery(api.auth.getCurrentUser);
  return <AuthUIView user={user} />;
}

export default withConvexProvider(AuthUIInner);
