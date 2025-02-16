import { Github } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"

function App() {
  return (
    <div className="dark min-h-screen bg-black flex items-center justify-center">
      <Card className="-mt-4 bg-card w-full max-w-sm mx-4">
        <CardHeader className="text-center">
          <CardTitle className="text-2xl text-white">OpenAgents Chat</CardTitle>
        </CardHeader>
        <CardContent>
          <Button className="w-full" size="lg">
            <Github />
            Log in with GitHub
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}

export default App;
