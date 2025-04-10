import { useState } from "react";
import { Form } from "react-router";
import type { ActionFunctionArgs } from "react-router";
import type { Route } from "./+types/spawn";
import { Button } from "~/components/ui/button";
import { Input } from "~/components/ui/input";
import { Label } from "~/components/ui/label";
import { Textarea } from "~/components/ui/textarea";

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

  // Here you would handle the form submission
  console.log({ githubToken, agentPurpose });

  // For now, just return the values
  return { success: true, data: { githubToken, agentPurpose } };
}

export default function Spawn() {
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleSubmit = (e: React.FormEvent) => {
    setIsSubmitting(true);
    // Form will be handled by the action function
  };

  return (
    <>
      <header className="w-full p-4 border-b">
        <div className="max-w-7xl mx-auto flex items-center">
          <a href="/" className="text-lg font-semibold hover:text-primary transition-colors">
            OpenAgents
          </a>
        </div>
      </header>

      <main className="w-full max-w-2xl mx-auto p-8 pt-16">
        <h1 className="text-4xl font-bold mb-12">Spawn a coding agent</h1>

        <div className="space-y-10">
          <Form method="post" onSubmit={handleSubmit} className="space-y-8">
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
                required
                autoComplete="new-password"
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
              />
              <p className="text-xs text-muted-foreground">
                Be specific about what you want the agent to work on. You'll be able to modify this later.
              </p>
            </div>

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
