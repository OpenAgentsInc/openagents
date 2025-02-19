import { useEffect, useState } from "react"
import { Label } from "~/components/ui/label"
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue
} from "~/components/ui/select"
import { Textarea } from "~/components/ui/textarea"

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

  return (
    <div className="flex flex-col items-center justify-center h-full p-8">
      <h1 className="w-full text-2xl flex-col tracking-tight @sm:text-3xl text-primary flex items-center justify-center text-center">
        {authState.authenticated
          ? `Welcome, ${authState.user?.metadata.name}`
          : "Welcome to OpenAgents Chat"}
        <span className="text-muted-foreground">
          What should we work on today?
        </span>
      </h1>

      <div className="w-full max-w-2xl space-y-6">
        <div className="flex items-center gap-4">
          <Label htmlFor="repo-select" className="whitespace-nowrap">
            Optional: Select a repo
          </Label>
          <Select>
            <SelectTrigger id="repo-select" className="w-full">
              <SelectValue placeholder="Choose a repository..." />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="openagents">openagents</SelectItem>
              <SelectItem value="other">other repository</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <Textarea
          placeholder="Type your message here..."
          className="w-full h-32"
        />
      </div>
    </div>
  );
}
