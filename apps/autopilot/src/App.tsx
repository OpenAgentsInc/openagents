import "./App.css";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

function App() {
  return (
    <main className="dark shell grid place-items-center p-4">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle>shadcn/ui</CardTitle>
          <CardDescription>Card component rendered in Tauri.</CardDescription>
        </CardHeader>
        <CardContent>Preset bddBV8Pw is installed.</CardContent>
      </Card>
    </main>
  );
}

export default App;
