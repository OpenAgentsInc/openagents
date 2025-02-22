import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { ChatInput } from "~/components/chat/chat-input";
import { v4 as uuid } from "uuid";

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

  useEffect(() => {
    fetch("/api/user")
      .then((res) => res.json())
      .then(setAuthState)
      .catch((error) => console.error("Error fetching user info:", error));
  }, []);

  const handleSubmit = async (message: string, repos?: string[]) => {
    try {
      // Generate a new chat ID
      const chatId = uuid();

      // Navigate immediately to show loading state
      navigate(`/chat/${chatId}`);

      // Send initial message via HTTP API
      const response = await fetch('/api/start-repo-chat', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          id: chatId,
          message,
          repos: repos || [],
          scope: "chat",
          use_reasoning: true,
        }),
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(error);
      }
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
      </div>
    </div>
  );
}