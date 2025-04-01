import React from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui";

export default function PromptsPage() {
  return (
    <Card className="font-mono">
      <CardHeader>
        <CardTitle>Custom Prompts</CardTitle>
        <CardDescription>
          Configure custom prompt templates for different tasks
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="py-6 text-center text-muted-foreground">
          Prompt management coming soon...
        </div>
      </CardContent>
    </Card>
  );
}