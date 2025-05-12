import { Link } from "react-router";
import { Button } from "~/components/ui/button";
import { Plus } from "lucide-react";
import { AgentDropdown } from "~/components/agent-dropdown";
import { useSession, signOut } from "~/lib/auth-client";

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

  console.log("Current user session:", session);

  return (
    <header className="sticky top-0 z-50 w-full border-b border-border/40 bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="container flex h-14 max-w-screen-2xl items-center">
        <div className="mr-4 hidden md:flex">
          <Link to="/" className="mr-6 flex items-center space-x-2">
            {/* <Icons.logo className="h-6 w-6" /> */}
            <span className="hidden font-bold sm:inline-block">
              OpenAgents
            </span>
          </Link>
          <nav className="flex items-center gap-6 text-sm">
            {/* Add other nav links here if needed */}
          </nav>
        </div>
        <div className="flex flex-1 items-center justify-between space-x-2 md:justify-end">
          <nav className="flex items-center">
            {isPending ? (
              <Button variant="outline" size="sm" disabled>
                Loading...
              </Button>
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
              <Button variant="ghost" asChild>
                <Link to="/login">Login</Link>
              </Button>
            )}
          </nav>
        </div>
      </div>
    </header>
  );
}
