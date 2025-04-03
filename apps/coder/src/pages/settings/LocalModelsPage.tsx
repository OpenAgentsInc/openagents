import React from "react";
import {
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  Alert,
  AlertTitle,
  AlertDescription
} from "@/components/ui";
import { InfoIcon } from "lucide-react";

export default function LocalModelsPage() {
  return (
    <>
      <CardHeader>
        <CardTitle>Local Models</CardTitle>
        <CardDescription>
          Connect to locally hosted AI models
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-6">
        <Alert variant="warning">
          <InfoIcon className="h-4 w-4" />
          <AlertTitle>Feature Disabled</AlertTitle>
          <AlertDescription>
            <p className="mb-2">Local model support is currently disabled to prevent connection issues.</p>
            <p>Please use cloud-based models like Anthropic Claude or OpenAI GPT models instead.</p>
          </AlertDescription>
        </Alert>
      </CardContent>
    </>
  );
}