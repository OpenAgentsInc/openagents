import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

function App() {
  return (
    <div className="min-h-screen bg-black flex items-center justify-center">
      <Card className="bg-zinc-900 border-zinc-800 w-full max-w-sm mx-4">
        <CardContent className="pt-6 text-center">
          <h1 className="text-4xl font-bold mb-6 text-white">OpenAgents</h1>
          <Button variant="outline" className="w-full">
            Log in with GitHub
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}

export default App;
