import type { Route } from "../+types/root";
import { Github } from "lucide-react"
import { Button } from "../components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card"

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Login" },
    { name: "description", content: "Login to OpenAgents" },
  ];
}

export default function Login() {
  const handleGitHubLogin = () => {
    // Use window.location for auth routes to bypass React Router
    window.location.href = '/auth/github/login';
  };

  return (
    <div className="flex justify-center items-center min-h-[50vh]">
      <Card className="w-full max-w-sm">
        <CardHeader className="text-center">
          <CardTitle className="text-xl text-white">
            Log in to OpenAgents
          </CardTitle>
        </CardHeader>
        <CardContent>
          <Button 
            className="w-full" 
            size="lg" 
            variant="nav"
            onClick={handleGitHubLogin}
          >
            <Github className="w-5 h-5" />
            Log in with GitHub
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}