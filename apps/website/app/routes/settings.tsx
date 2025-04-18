import { useState, useEffect } from "react";
import { useSession } from "~/lib/auth-client";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "~/components/ui/card";
import { Input } from "~/components/ui/input";
import { Button } from "~/components/ui/button";
import { Label } from "~/components/ui/label";
import { Eye, EyeOff, Github, Save, CheckCircle, ExternalLink } from "lucide-react";
import { HeaderSettings } from "~/components/layout/headers/settings/header";
import MainLayout from "@/components/layout/main-layout";

/**
 * Settings page component for OpenAgents
 */
export default function SettingsPage() {
  const { data: session } = useSession();
  const [token, setToken] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load token from local storage on component mount
  useEffect(() => {
    const storedToken = localStorage.getItem("github_token");
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
    // Save to local storage
    localStorage.setItem("github_token", token);
    setSaved(true);

    // Dispatch event to notify other components
    const event = new Event("github-token-changed");
    window.dispatchEvent(event);
  };

  const handleClearToken = () => {
    localStorage.removeItem("github_token");
    setToken("");
    setSaved(false);
  };

  return (
    <MainLayout header={<HeaderSettings />}>
      <div className="flex-1 container mx-auto p-4">
        <div className="mb-6 mt-4">
          <h1 className="text-2xl font-bold tracking-tight">Account Settings</h1>
          <p className="text-muted-foreground mt-1">
            Manage your account settings and configure integrations
          </p>
        </div>

        <div className="grid gap-6 max-w-3xl">
          {/* GitHub Integration Card */}
          <Card>
            <CardHeader className="pb-3">
              <div className="flex items-center gap-2">
                <Github className="h-5 w-5" />
                <CardTitle>GitHub Integration</CardTitle>
              </div>
              <CardDescription>
                Connect your GitHub account to enable issue management, repository access, and other GitHub features
              </CardDescription>
            </CardHeader>
            <CardContent className="pb-4">
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="github-token">Personal Access Token</Label>
                  <div className="relative">
                    <Input
                      id="github-token"
                      type={showToken ? "text" : "password"}
                      placeholder="ghp_yourgithubtoken..."
                      value={token}
                      onChange={handleTokenChange}
                      className="pr-10"
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
                  {error && (
                    <p className="text-sm text-destructive mt-1">{error}</p>
                  )}
                  <p className="text-sm text-muted-foreground mt-1">
                    Token is stored securely in your browser's local storage and never shared with our servers.
                  </p>
                </div>

                <div className="space-y-2">
                  <h4 className="text-sm font-medium">How to create a token</h4>
                  <ol className="list-decimal ml-4 space-y-1 text-sm text-muted-foreground">
                    <li>Go to GitHub → Settings → Developer settings</li>
                    <li>Choose Personal access tokens → Fine-grained tokens</li>
                    <li>Create a new token with appropriate repository permissions</li>
                  </ol>
                  <a
                    href="https://github.com/settings/tokens"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm flex items-center gap-1 text-primary hover:underline mt-2"
                  >
                    Go to GitHub token settings
                    <ExternalLink className="h-3 w-3" />
                  </a>
                </div>
              </div>
            </CardContent>
            <CardFooter className="flex justify-between gap-2 border-t bg-muted/50 pt-6">
              <Button 
                variant="outline" 
                onClick={handleClearToken}
                disabled={!token}
              >
                Clear Token
              </Button>
              <Button 
                onClick={handleSaveToken}
                disabled={!token || saved}
                className="flex items-center gap-1"
              >
                {saved ? <CheckCircle className="h-4 w-4" /> : <Save className="h-4 w-4" />}
                {saved ? "Saved" : "Save Token"}
              </Button>
            </CardFooter>
          </Card>
        </div>
      </div>
    </MainLayout>
  );
}