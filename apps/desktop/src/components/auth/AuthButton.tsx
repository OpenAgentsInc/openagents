import React from 'react';
import { Button } from '@/components/ui/button';
import { useAuth } from '@/contexts/AuthContext';

export const AuthButton: React.FC = () => {
  const { user, isAuthenticated, isLoading, login, logout } = useAuth();

  if (isLoading) {
    return (
      <Button disabled size="sm" variant="outline">
        Loading...
      </Button>
    );
  }

  if (isAuthenticated && user) {
    return (
      <div className="flex items-center gap-2">
        <div className="flex items-center gap-2 text-sm">
          {user.avatar && (
            <img 
              src={user.avatar} 
              alt={user.name || user.githubUsername}
              className="w-6 h-6 rounded-full"
            />
          )}
          <span className="text-muted-foreground">
            {user.name || user.githubUsername}
          </span>
        </div>
        <Button onClick={logout} size="sm" variant="outline">
          Logout
        </Button>
      </div>
    );
  }

  return (
    <Button onClick={login} size="sm">
      Login with GitHub
    </Button>
  );
};