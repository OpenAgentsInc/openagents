import { useEffect, useState } from "react";
import { Link } from "react-router";
import { Button } from "./ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";

interface UserMetadata {
  name: string;
  login: string;
  avatar_url: string;
}

interface User {
  id: number;
  metadata: UserMetadata;
  pseudonym: string | null;
}

interface AuthState {
  authenticated: boolean;
  user: User | null;
}

export function HeaderBar() {
  const [authState, setAuthState] = useState<AuthState>({
    authenticated: false,
    user: null,
  });

  useEffect(() => {
    fetch("/api/user")
      .then((res) => res.json())
      .then(setAuthState)
      .catch((error) => console.error("Error fetching user info:", error));
  }, []);

  const navigateTo = (path: string) => {
    window.location.href = path;
  };

  return (
    <div className="draggable no-draggable-children sticky top-0 p-3 flex items-center justify-between z-10 h-header-height font-semibold bg-background border-b border-zinc-800">
      {/* Centered empty div for future content or alignment */}
      <div className="absolute start-1/2 ltr:-translate-x-1/2 rtl:translate-x-1/2"></div>

      {/* Left/Middle Section */}
      <div className="flex items-center gap-0 overflow-hidden">
        <div className="flex items-center h-10">
          <span className="mt-1 flex items-center h-full" data-state="closed">
            <Link to="/chat" className="h-full">
              <button
                aria-label="New chat"
                className="cursor-pointer h-full rounded-lg px-2 text-token-text-secondary focus-visible:bg-token-surface-hover focus-visible:outline-0 enabled:hover:bg-token-surface-hover disabled:text-token-text-quaternary flex items-center"
              >
                <svg
                  width="24"
                  height="24"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                  xmlns="http://www.w3.org/2000/svg"
                  className="icon-xl-heavy"
                >
                  <path
                    d="M15.6729 3.91287C16.8918 2.69392 18.8682 2.69392 20.0871 3.91287C21.3061 5.13182 21.3061 7.10813 20.0871 8.32708L14.1499 14.2643C13.3849 15.0293 12.3925 15.5255 11.3215 15.6785L9.14142 15.9899C8.82983 16.0344 8.51546 15.9297 8.29289 15.7071C8.07033 15.4845 7.96554 15.1701 8.01005 14.8586L8.32149 12.6785C8.47449 11.6075 8.97072 10.615 9.7357 9.85006L15.6729 3.91287ZM18.6729 5.32708C18.235 4.88918 17.525 4.88918 17.0871 5.32708L11.1499 11.2643C10.6909 11.7233 10.3932 12.3187 10.3014 12.9613L10.1785 13.8215L11.0386 13.6986C11.6812 13.6068 12.2767 13.3091 12.7357 12.8501L18.6729 6.91287C19.1108 6.47497 19.1108 5.76499 18.6729 5.32708Z"
                    fill="currentColor"
                  />
                </svg>
              </button>
            </Link>
          </span>
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              aria-label="Model switcher"
              type="button"
              className="group flex cursor-pointer items-center gap-1 rounded-lg px-3 text-lg hover:bg-token-main-surface-secondary radix-state-open:bg-token-main-surface-secondary font-semibold text-token-text-secondary overflow-hidden whitespace-nowrap h-10 focus:outline-none focus-visible:outline-none focus-visible:ring-0 focus:ring-0 focus:ring-offset-0"
              style={{ viewTransitionName: "var(--vt-thread-model-switcher)" }}
            >
              <div className="text-token-text-secondary select-none">
                ⏻ OpenAgents
              </div>
              <svg
                width="24"
                height="24"
                viewBox="0 0 24 24"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
                className="icon-md text-token-text-tertiary"
              >
                <path
                  fillRule="evenodd"
                  clipRule="evenodd"
                  d="M5.29289 9.29289C5.68342 8.90237 6.31658 8.90237 6.70711 9.29289L12 14.5858L17.2929 9.29289C17.6834 8.90237 18.3166 8.90237 18.7071 9.29289C19.0976 9.68342 19.0976 10.3166 18.7071 10.7071L12.7071 16.7071C12.5196 16.8946 12.2652 17 12 17C11.7348 17 11.4804 16.8946 11.2929 16.7071L5.29289 10.7071C4.90237 10.3166 4.90237 9.68342 5.29289 9.29289Z"
                  fill="currentColor"
                />
              </svg>
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem asChild>
              <Link to="/">Homepage</Link>
            </DropdownMenuItem>
            <DropdownMenuItem asChild>
              <a
                href="https://github.com/OpenAgentsInc/openagents"
                target="_blank"
                rel="noopener noreferrer"
              >
                GitHub
              </a>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {/* Right Section */}
      <div className="gap-2 flex items-center pr-1 leading-[0]">
        {authState.authenticated ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <div className="flex items-center gap-2 cursor-pointer hover:bg-token-surface-hover rounded-lg px-2 py-1">
                <img
                  src={authState.user?.metadata.avatar_url}
                  alt="Avatar"
                  className="w-8 h-8 rounded-full select-none"
                />
                <span className="text-sm text-foreground select-none">
                  {authState.user?.metadata.name}
                </span>
                <svg
                  width="24"
                  height="24"
                  viewBox="0 0 24 24"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  className="icon-md text-token-text-tertiary"
                >
                  <path
                    fillRule="evenodd"
                    clipRule="evenodd"
                    d="M5.29289 9.29289C5.68342 8.90237 6.31658 8.90237 6.70711 9.29289L12 14.5858L17.2929 9.29289C17.6834 8.90237 18.3166 8.90237 18.7071 9.29289C19.0976 9.68342 19.0976 10.3166 18.7071 10.7071L12.7071 16.7071C12.5196 16.8946 12.2652 17 12 17C11.7348 17 11.4804 16.8946 11.2929 16.7071L5.29289 10.7071C4.90237 10.3166 4.90237 9.68342 5.29289 9.29289Z"
                    fill="currentColor"
                  />
                </svg>
              </div>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
              <DropdownMenuItem onClick={() => navigateTo("/auth/logout")}>
                Log out
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ) : (
          <div className="flex items-center justify-center gap-2">
            <Button
              variant="secondary"
              onClick={() => navigateTo("/login")}
              data-testid="login-button"
            >
              Log in
            </Button>
            <Button
              variant="default"
              className="screen-arch:hidden md:screen-arch:flex"
              onClick={() => navigateTo("/signup")}
              data-testid="signup-button"
            >
              Sign up
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

export default HeaderBar;
