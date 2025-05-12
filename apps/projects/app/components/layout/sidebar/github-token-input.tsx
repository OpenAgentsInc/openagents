import * as React from 'react';
import { useState, useEffect } from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { RiGithubLine } from '@remixicon/react';
import { CheckIcon, KeyIcon } from 'lucide-react';

export function GitHubTokenInput() {
  const [token, setToken] = useState('');
  const [showToken, setShowToken] = useState(false);
  const [savedToken, setSavedToken] = useState('');
  const [dialogOpen, setDialogOpen] = useState(false);

  // Load saved token on component mount
  useEffect(() => {
    if (typeof window !== 'undefined' && window.localStorage) {
      const storedToken = localStorage.getItem('github_token') || '';
      setSavedToken(storedToken);
      setToken(storedToken);
    }
  }, []);

  const handleSaveToken = () => {
    if (typeof window !== 'undefined' && window.localStorage) {
      localStorage.setItem('github_token', token);
      setSavedToken(token);
      setDialogOpen(false);
    }
  };

  const clearToken = () => {
    setToken('');
    if (typeof window !== 'undefined' && window.localStorage) {
      localStorage.removeItem('github_token');
      setSavedToken('');
    }
  };

  const tokenPreview = savedToken
    ? `${savedToken.substring(0, 4)}...${savedToken.substring(savedToken.length - 4)}`
    : 'Not set';

  return (
    <div className="px-3 py-2">
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            className="w-full justify-start gap-2 text-xs"
          >
            <RiGithubLine className="h-4 w-4" />
            <span className="flex-1 text-left">GitHub Token: {tokenPreview}</span>
            <KeyIcon className="h-3 w-3 text-muted-foreground" />
          </Button>
        </DialogTrigger>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>GitHub Personal Access Token</DialogTitle>
            <DialogDescription>
              Enter your GitHub Personal Access Token to enable integration with GitHub tools.
              You'll need a token with repo scope for most operations.
            </DialogDescription>
          </DialogHeader>

          <div className="my-4 space-y-4">
            <div className="space-y-2">
              <div className="flex items-center">
                <Input
                  type={showToken ? "text" : "password"}
                  placeholder="ghp_your_github_token"
                  value={token}
                  onChange={(e) => setToken(e.target.value)}
                  className="flex-1"
                />
                <Button
                  variant="ghost"
                  size="sm"
                  type="button"
                  onClick={() => setShowToken(!showToken)}
                  className="ml-2"
                >
                  {showToken ? "Hide" : "Show"}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Token is stored in your browser's local storage. Never shared with our servers.
              </p>
            </div>

            <div>
              <h4 className="text-sm font-medium mb-1">How to create a token</h4>
              <ol className="list-decimal text-xs text-muted-foreground ml-4 space-y-1">
                <li>Go to GitHub → Settings → Developer settings</li>
                <li>Choose Personal access tokens → Fine-grained tokens</li>
                <li>Create a new token with repo permissions</li>
              </ol>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" size="sm" onClick={clearToken}>
              Clear Token
            </Button>
            <Button onClick={handleSaveToken} disabled={!token}>
              Save Token
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
