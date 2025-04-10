import { useState, useEffect } from "react";
import { Form, useNavigate, useActionData } from "react-router";
import type { ActionFunctionArgs } from "react-router";
import type { Route } from "./+types/spawn";
import { Button } from "~/components/ui/button";
import { Input } from "~/components/ui/input";
import { Label } from "~/components/ui/label";
import { Textarea } from "~/components/ui/textarea";
import { useAgentStore } from "~/lib/store";
import { Header } from "~/components/header";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Spawn Agent - OpenAgents" },
    { name: "description", content: "Spawn a new coding agent" },
  ];
}

export async function action({ request }: ActionFunctionArgs) {
  const formData = await request.formData();
  const githubToken = formData.get("githubToken") as string;
  const agentPurpose = formData.get("agentPurpose") as string;

  // Here you would handle the form submission with server-side logic
  console.log({ githubToken, agentPurpose });

  // Validate the form data
  if (!githubToken || !agentPurpose) {
    return { success: false, error: "GitHub Token and Agent Purpose are required" };
  }
  
  // Validate GitHub token format
  if (!githubToken.startsWith('github_pat_')) {
    return { success: false, error: "GitHub Token must begin with 'github_pat_'" };
  }

  // In a real implementation, you'd communicate with your agent service here
  // For example: const result = await agentService.createAgent(githubToken, agentPurpose);

  // For now, simulate success and return form data
  // Important: Don't return the GitHub token for security
  return {
    success: true,
    data: {
      id: `agent-${Date.now().toString(36)}-${Math.random().toString(36).substring(2, 7)}`,
      purpose: agentPurpose
    }
  };
}

export default function Spawn() {
  // Get action data from the form submission
  const actionData = useActionData<{
    success: boolean;
    error?: string;
    data?: { id: string; purpose: string }
  }>();

  const navigate = useNavigate();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const { agentPurpose, setGithubToken, setAgentPurpose } = useAgentStore();

  // Form state
  const [githubToken, setGithubTokenLocal] = useState("");
  const [purpose, setPurpose] = useState("");
  const [error, setError] = useState<string | null>(null);

  // Load initial purpose from store
  useEffect(() => {
    if (agentPurpose) {
      setPurpose(agentPurpose);
    }
  }, [agentPurpose]);

  // Handle form submission success or error
  useEffect(() => {
    if (actionData) {
      if (actionData.success && actionData.data?.id) {
        const agentId = actionData.data.id;
        
        // Add agent to store with the ID timestamp to avoid hydration issues
        const createdAt = parseInt(agentId.split('-')[1], 36);
        useAgentStore.getState().addAgent({
          id: agentId,
          purpose: actionData.data.purpose,
          createdAt: createdAt || 0
        });
        
        // Reset isSubmitting
        setIsSubmitting(false);
        
        // Navigate to agent details page
        setTimeout(() => {
          navigate(`/agent/${agentId}`);
        }, 100); // Small delay to ensure store is updated
      } else if (actionData.error) {
        setError(actionData.error);
        setIsSubmitting(false);
      }
    }
  }, [actionData, navigate]);

  // Client-side validation
  const validateForm = () => {
    if (!githubToken) {
      setError("GitHub Token is required");
      return false;
    }
    
    if (!githubToken.startsWith('github_pat_')) {
      setError("GitHub Token must begin with 'github_pat_'");
      return false;
    }
    
    if (!purpose) {
      setError("Agent Purpose is required");
      return false;
    }
    
    return true;
  };
  
  // Handle client-side form submission updates
  const handleBeforeSubmit = () => {
    setError(null);
    
    // Client-side validation
    if (!validateForm()) {
      return;
    }
    
    setIsSubmitting(true);
    
    // Save to Zustand store
    setGithubToken(githubToken);
    setAgentPurpose(purpose);
  };

  return (
    <>
      <Header showNewAgentButton={false} />

      <main className="w-full max-w-2xl mx-auto p-8 pt-24">
        <h1 className="text-3xl font-bold mb-8">Spawn a coding agent</h1>

        <div className="space-y-10">
          <Form
            method="post"
            className="space-y-8"
            onSubmit={handleBeforeSubmit}
            preventScrollReset>
            {/* Hidden username field for accessibility */}
            <div className="sr-only">
              <Label htmlFor="username">GitHub Username</Label>
              <Input
                id="username"
                name="username"
                type="text"
                autoComplete="username"
                tabIndex={-1}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="githubToken">GitHub Token</Label>
              <Input
                id="githubToken"
                name="githubToken"
                type="password"
                placeholder="github_pat_*********************************"
                pattern="^github_pat_.*"
                required
                autoComplete="new-password"
                value={githubToken}
                onChange={(e) => setGithubTokenLocal(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                Create a <a href="https://github.com/settings/personal-access-tokens" target="_blank" rel="noopener noreferrer" className="underline hover:text-foreground">fine-grained GitHub token</a> with only the permissions this agent needs. Your token will not be stored.
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="agentPurpose">Agent Purpose</Label>
              <Textarea
                id="agentPurpose"
                name="agentPurpose"
                placeholder="Describe what you want this agent to help you with..."
                rows={5}
                required
                value={purpose}
                onChange={(e) => setPurpose(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                Be specific about what you want the agent to work on. You'll be able to modify this later.
              </p>
            </div>

            {/* Display any validation errors */}
            {error && (
              <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">
                {error}
              </div>
            )}

            {/* Display success message if form was submitted successfully */}
            {actionData?.success && (
              <div className="p-3 rounded-md bg-green-500/10 text-green-600 dark:text-green-400 text-sm">
                Agent created successfully! ID: {actionData.data?.id}
              </div>
            )}

            <Button
              type="submit"
              className="w-full mt-4"
              disabled={isSubmitting}
            >
              {isSubmitting ? "Creating Agent..." : "Spawn Agent"}
            </Button>
          </Form>
        </div>
      </main>
    </>
  );
}
