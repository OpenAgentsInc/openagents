import { useState, useEffect } from "react";
import { Input } from "~/components/ui/input";
import { Eye, EyeOff, Save, CheckCircle, Github } from "lucide-react";

const TOKEN_STORAGE_KEY = "github_token";
const TOKEN_PREFIX = "github_pat_";

export function GitHubTokenInput() {
  const [token, setToken] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);

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

  // Compact mode that only shows status
  if (!expanded && saved) {
    return (
      <div 
        className="mb-3 border rounded-md p-2 bg-background cursor-pointer flex items-center justify-between"
        onClick={() => setExpanded(true)}
      >
        <div className="flex items-center gap-2">
          <Github className="h-3.5 w-3.5 text-green-600 dark:text-green-400" />
          <span className="text-xs">GitHub token saved</span>
        </div>
        <button className="text-xs text-muted-foreground hover:text-foreground">Edit</button>
      </div>
    );
  }

  // Expanded input form
  return (
    <div className="mb-3 border rounded-md p-2 bg-background">
      <div className="flex items-center justify-between mb-1.5">
        <div className="flex items-center gap-1.5">
          <Github className="h-3.5 w-3.5" />
          <span className="text-xs font-medium">GitHub Token</span>
        </div>
        {saved && (
          <button 
            className="text-[10px] text-muted-foreground hover:text-foreground"
            onClick={() => setExpanded(false)}
          >
            Collapse
          </button>
        )}
      </div>

      <div className="flex gap-1.5">
        <div className="relative flex-1">
          <Input
            id="github-token"
            placeholder="github_pat_..."
            value={token}
            onChange={handleTokenChange}
            type={showToken ? "text" : "password"}
            className="pr-7 font-mono text-xs h-7 py-0"
          />
          <button 
            type="button"
            onClick={() => setShowToken(!showToken)}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            aria-label={showToken ? "Hide token" : "Show token"}
          >
            {showToken ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
          </button>
        </div>
        <button
          onClick={handleSaveToken}
          className="px-2 h-7 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 flex items-center gap-1 disabled:opacity-50"
          disabled={!token || saved}
        >
          {saved ? <CheckCircle className="h-3 w-3" /> : <Save className="h-3 w-3" />}
          <span className="text-[10px]">{saved ? "Saved" : "Save"}</span>
        </button>
      </div>
      
      {error && (
        <p className="text-[10px] text-destructive mt-1">{error}</p>
      )}
      
      <p className="text-[9px] text-muted-foreground mt-1">
        Must start with github_pat_ and have repo:read access
      </p>
    </div>
  );
}
