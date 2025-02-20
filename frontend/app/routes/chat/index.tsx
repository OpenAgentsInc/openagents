import { useAgentSync } from "agentsync"
import { useEffect, useState } from "react"
import { useNavigate } from "react-router"
import { ChatInput } from "~/components/chat/chat-input"

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
  const navigate = useNavigate();
  const [authState, setAuthState] = useState<AuthState>({
    authenticated: false,
    user: null,
  });

  const { sendMessage, state } = useAgentSync({
    scope: "chat",
  });

  useEffect(() => {
    fetch("/api/user")
      .then((res) => res.json())
      .then(setAuthState)
      .catch((error) => console.error("Error fetching user info:", error));
  }, []);

  const handleSubmit = async (message: string, repos?: string[]) => {
    try {
      // Navigate immediately to show loading state
      const response = await sendMessage(message, repos);
      navigate(`/chat/${response.id}`);
    } catch (error) {
      console.error("Error starting chat:", error);
      // Could add error handling UI here
    }
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
        {!state.isOnline && (
          <div className="mt-2 text-sm text-red-500">
            You are currently offline. Messages will be queued.
          </div>
        )}
        {state.pendingChanges > 0 && (
          <div className="mt-2 text-sm text-yellow-500">
            {state.pendingChanges} pending changes to sync
          </div>
        )}
      </div>
    </div>
  );
}
