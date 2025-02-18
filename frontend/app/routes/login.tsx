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
  return (
    <div className="fixed inset-0 dark bg-black flex items-center justify-center">
      <Card className="-mt-12 w-full max-w-sm mx-4">
        <CardHeader className="text-center">
          <CardTitle className="text-xl text-white">
            Log in to OpenAgents
          </CardTitle>
        </CardHeader>
        <CardContent>
          <Button className="w-full" size="lg" variant="nav">
            <Github className="w-5 h-5" />
            Log in with GitHub
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
