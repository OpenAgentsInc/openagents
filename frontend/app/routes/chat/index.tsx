import { useEffect, useState } from "react"
import { ChatInput } from "~/components/chat-input"

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

export default function ChatIndex() {
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

  const handleSubmit = (message: string, repo: string | undefined) => {
    // Handle the chat submission here
    console.log("Message:", message, "Repo:", repo);
  };

  return (
    <div className="flex flex-col items-center justify-center h-full p-8">
      <h1 className="-mt-8 w-full text-2xl flex-col tracking-tight leading-[2.8rem] sm:text-3xl text-primary flex items-center justify-center text-center">
        {authState.authenticated
          ? `Welcome ${authState.user?.metadata.name}!`
          : "Welcome to OpenAgents Chat"}
        <span className="text-muted-foreground">
          What should we work on today?
        </span>
      </h1>

      <div className="w-full max-w-2xl mt-12">
        <ChatInput onSubmit={handleSubmit} />
      </div>
    </div>
  );
}
