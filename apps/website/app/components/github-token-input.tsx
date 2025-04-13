import { useState, useEffect } from "react";
import { Input } from "~/components/ui/input";
import { Label } from "~/components/ui/label";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "~/components/ui/card";
import { Eye, EyeOff, Save, CheckCircle } from "lucide-react";

const TOKEN_STORAGE_KEY = "github_token";
const TOKEN_PREFIX = "github_pat_";

export function GitHubTokenInput() {
  const [token, setToken] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load token from local storage on component mount
  useEffect(() => {
    const storedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
    if (storedToken) {
      setToken(storedToken);
      setSaved(true);
    }
  }, []);

  const handleTokenChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newToken = e.target.value;
    setToken(newToken);
    setSaved(false);

    // Clear error if they start typing
    if (error) {
      setError(null);
    }
  };

  const handleSaveToken = () => {
    // Validate token format
    if (!token.startsWith(TOKEN_PREFIX)) {
      setError(`Token must start with "${TOKEN_PREFIX}"`);
      return;
    }

    // Save to local storage
    localStorage.setItem(TOKEN_STORAGE_KEY, token);
    setSaved(true);

    // Dispatch event to notify other components
    const event = new Event("github-token-changed");
    window.dispatchEvent(event);
  };

  return (
    <Card className="mb-4">
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">GitHub Token</CardTitle>
        <CardDescription className="text-xs">
          Add your GitHub Personal Access Token to use GitHub tools
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-2">
          <div className="space-y-1">
            <Label htmlFor="github-token" className="text-xs">Personal Access Token</Label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Input
                  id="github-token"
                  placeholder="github_pat_..."
                  value={token}
                  onChange={handleTokenChange}
                  type={showToken ? "text" : "password"}
                  className="pr-10 font-mono text-xs"
                />
                <button
                  type="button"
                  onClick={() => setShowToken(!showToken)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  aria-label={showToken ? "Hide token" : "Show token"}
                >
                  {showToken ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
              <button
                onClick={handleSaveToken}
                className="px-3 py-2 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 flex items-center gap-1 disabled:opacity-50"
                disabled={!token || saved}
              >
                {saved ? <CheckCircle className="h-3.5 w-3.5" /> : <Save className="h-3.5 w-3.5" />}
                {saved ? "Saved" : "Save"}
              </button>
            </div>
            {error && (
              <p className="text-xs text-destructive mt-1">{error}</p>
            )}
            {saved && (
              <p className="text-xs text-green-600 dark:text-green-400 mt-1">Token saved!</p>
            )}
          </div>
          <p className="text-[10px] text-muted-foreground">
            Token must start with github_pat_ and have at least repo:read access.
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
