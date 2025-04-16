import { Link } from "react-router";
import { Button } from "~/components/ui/button";
import { Plus } from "lucide-react";
import { AgentDropdown } from "~/components/agent-dropdown";
import { signOut, useSession } from "~/lib/auth-client";

export function Header({ showNewAgentButton = true }: { showNewAgentButton?: boolean }) {
  const { data: session, isPending } = useSession();

  const handleSignOut = async () => {
    await signOut({
      fetchOptions: {
        onSuccess: () => {
          window.location.href = "/";
        }
      }
    });
  };

  return (
    <header className="w-full p-4 border-b fixed top-0 z-10 bg-background h-16 flex items-center">
      <div className="max-w-7xl mx-auto flex items-center justify-between w-full">
        <Link to="/" className="text-lg font-semibold hover:text-primary transition-colors select-none">
          OpenAgents
        </Link>

        <div className="h-full flex items-center gap-2">
          {/* Only show agent-related UI if user is logged in */}
          {/* {!isPending && session?.user && (
            <>
              <AgentDropdown />

              {showNewAgentButton && (
                <Button variant="outline" asChild>
                  <Link to="/spawn" className="flex items-center gap-2">
                    <Plus size={16} />
                    <span>Spawn coding agent</span>
                  </Link>
                </Button>
              )}
            </>
          )} */}

          {isPending ? (
            <></>
          ) : session?.user ? (
            <div className="flex items-center gap-2">
              <div className="text-sm mr-2">
                {session.user.name || session.user.email}
              </div>
              <Button variant="ghost" onClick={handleSignOut}>
                Logout
              </Button>
            </div>
          ) : (
            <div className="flex items-center gap-3">
              <Button variant="outline" size="sm" asChild>
                <Link to="/login">Log in</Link>
              </Button>
              <Button variant="default" size="sm" asChild>
                <Link to="/signup">Sign up</Link>
              </Button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
