import { Github } from "lucide-react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import ChatScreen from "./pages/ChatScreen";

function LoginScreen() {
  return (
    <div className="fixed inset-0 dark bg-black flex items-center justify-center">
      <Card className="-mt-12 w-full max-w-sm mx-4">
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

function App() {
  return (
    <BrowserRouter basename="/chat">
      <Routes>
        <Route path="/" element={<LoginScreen />} />
        <Route path="/new" element={<ChatScreen />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
