import React, { useState, useEffect } from "react";
import { useSettings, DEFAULT_SYSTEM_PROMPT } from "@openagents/core";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Button,
  Textarea,
  Alert,
  AlertDescription,
} from "@/components/ui";

export default function PromptsPage() {
  const { settings, isLoading, updateSettings, setPreference, getPreference } = useSettings();
  const [systemPrompt, setSystemPrompt] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError] = useState(false);

  // Load system prompt from settings when component mounts
  useEffect(() => {
    const loadSystemPrompt = async () => {
      try {
        // We store the system prompt as a preference with the default value
        const savedPrompt = await getPreference("defaultSystemPrompt", DEFAULT_SYSTEM_PROMPT);
        setSystemPrompt(savedPrompt);
      } catch (error) {
        console.error("Error loading system prompt:", error);
        // Fall back to default on error
        setSystemPrompt(DEFAULT_SYSTEM_PROMPT);
      }
    };

    loadSystemPrompt();
  }, [getPreference]);

  const handleSave = async () => {
    try {
      setIsSaving(true);
      setSaveSuccess(false);
      setSaveError(false);

      // Save the system prompt as a preference
      await setPreference("defaultSystemPrompt", systemPrompt);
      console.log("System prompt saved successfully:", systemPrompt);

      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000); // Hide success message after 3 seconds
    } catch (error) {
      console.error("Error saving system prompt:", error);
      setSaveError(true);
      setTimeout(() => setSaveError(false), 3000); // Hide error message after 3 seconds
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full font-mono">
        <p>Loading settings...</p>
      </div>
    );
  }

  return (
    <Card className="font-mono">
      <CardHeader>
        <CardTitle>Custom Prompts</CardTitle>
        <CardDescription>
          Configure custom prompt templates for different tasks
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <h3 className="text-lg font-medium">System Prompt</h3>
          <p className="text-sm text-muted-foreground">
            Set the default system prompt used for all new conversations
          </p>
          <Textarea
            value={systemPrompt}
            onChange={(e) => setSystemPrompt(e.target.value)}
            placeholder="Enter your system prompt here..."
            className="min-h-32"
          />
        </div>

        {saveSuccess && (
          <Alert variant="default" className="bg-green-500/10 text-green-500 border-green-500/20">
            <AlertDescription>
              System prompt saved successfully!
            </AlertDescription>
          </Alert>
        )}

        {saveError && (
          <Alert variant="destructive">
            <AlertDescription>
              Failed to save system prompt. Please try again.
            </AlertDescription>
          </Alert>
        )}

        <div className="flex space-x-2">
          <Button
            onClick={handleSave}
            className="mt-4"
            disabled={isSaving}
          >
            {isSaving ? "Saving..." : "Save System Prompt"}
          </Button>
          
          <Button 
            onClick={() => {
              setSystemPrompt(DEFAULT_SYSTEM_PROMPT);
              // Auto-save when reset is clicked
              setTimeout(() => {
                setPreference("defaultSystemPrompt", DEFAULT_SYSTEM_PROMPT)
                  .then(() => {
                    setSaveSuccess(true);
                    setTimeout(() => setSaveSuccess(false), 3000);
                  })
                  .catch(error => {
                    console.error("Error resetting system prompt:", error);
                    setSaveError(true);
                    setTimeout(() => setSaveError(false), 3000);
                  });
              }, 100);
            }} 
            variant="outline"
            className="mt-4"
          >
            Reset to Default
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
