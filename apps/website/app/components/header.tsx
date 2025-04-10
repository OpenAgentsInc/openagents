import { Link } from "react-router-dom";
import { Button } from "~/components/ui/button";
import { useSession, signOut } from "~/lib/auth-client"; // Import useSession and signOut
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "~/components/ui/dropdown-menu";
import { User } from "lucide-react";

export function Header() {
  const { data: session, isPending } = useSession(); // Get session state

  const handleSignOut = async () => {
    await signOut({
      fetchOptions: {
        // Redirect to home page on successful sign out
        onSuccess: () => window.location.replace("/"), 
      }
    });
  };

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
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="secondary" size="sm">
                    <User className="mr-2 h-4 w-4" />
                    {session.user.name || session.user.email}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuLabel>My Account</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  {/* Add links to profile/settings pages here if they exist */}
                  {/* <DropdownMenuItem asChild><Link to="/profile">Profile</Link></DropdownMenuItem> */}
                  {/* <DropdownMenuItem asChild><Link to="/settings">Settings</Link></DropdownMenuItem> */}
                  {/* <DropdownMenuSeparator /> */}
                  <DropdownMenuItem onClick={handleSignOut} className="cursor-pointer">
                    Log out
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <Button asChild variant="secondary" size="sm">
                <Link to="/login">Login</Link>
              </Button>
            )}
          </nav>
        </div>
      </div>
    </header>
  );
}
