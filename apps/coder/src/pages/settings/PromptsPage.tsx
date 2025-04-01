import React, { useState } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Button,
  Textarea,
} from "@/components/ui";

export default function PromptsPage() {
  const [systemPrompt, setSystemPrompt] = useState("");

  const handleSave = () => {
    console.log("Saving system prompt:", systemPrompt);
  };

  return (
    <Card className="font-mono">

      <CardContent className="space-y-4">
        <div className="space-y-2">
          <h3 className="text-lg font-medium">System Prompt</h3>
          <p className="text-sm text-muted-foreground">
            Set the default system prompt used for all conversations
          </p>
          <Textarea
            value={systemPrompt}
            onChange={(e) => setSystemPrompt(e.target.value)}
            placeholder="Enter your system prompt here..."
            className="min-h-32"
          />
        </div>
        <Button onClick={handleSave} className="mt-4">
          Save System Prompt
        </Button>
      </CardContent>
    </Card>
  );
}
