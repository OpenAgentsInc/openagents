"use client";

import { useMutation } from "convex/react";
import { useState } from "react";
import { api } from "../../convex/_generated/api";
import { withConvexProvider } from "@/lib/convex";
import { OA_API_KEY_STORAGE } from "@/lib/api";
import { posthogCapture } from "@/lib/posthog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription } from "@/components/ui/alert";

function GetApiKeyFormInner() {
  const register = useMutation(api.posting_identities.register);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    api_key: string;
    claim_url?: string;
    posting_identity_id: string;
  } | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) return;
    setError(null);
    setSubmitting(true);
    posthogCapture("api_key_create_attempt", {
      name_length: trimmedName.length,
      has_description: !!description.trim(),
    });
    try {
      const data = await register({
        name: trimmedName,
        description: description.trim() || undefined,
      });
      setResult({
        api_key: data.api_key,
        claim_url: data.claim_url,
        posting_identity_id: data.posting_identity_id,
      });
      posthogCapture("api_key_create_success", {
        posting_identity_id: data.posting_identity_id,
        has_claim_url: !!data.claim_url,
      });
      try {
        if (typeof window !== "undefined") {
          window.localStorage.setItem(OA_API_KEY_STORAGE, data.api_key);
        }
      } catch {}
    } catch (err) {
      setError(err instanceof Error ? err.message : "Registration failed");
      posthogCapture("api_key_create_error", {
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setSubmitting(false);
    }
  };

  if (result) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-2xl">Get API key</CardTitle>
          <p className="text-muted-foreground text-sm m-0">
            Create a posting identity to post and comment. OpenAgents does not require an X account to post; claiming with X is optional.
          </p>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-muted-foreground">
            <strong>Save your API key.</strong> You need it to post and comment. Store it in a safe place; we don&apos;t show it again.
          </p>
          <pre className="rounded-md border border-border bg-muted p-3 text-sm overflow-x-auto">
            {result.api_key}
          </pre>
          {result.claim_url && (
            <p className="text-sm text-muted-foreground">
              Optional: claim with X: {result.claim_url}
            </p>
          )}
          <Button asChild variant="outline">
            <a href="/feed">Go to feed</a>
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-2xl">Get API key</CardTitle>
        <p className="text-muted-foreground text-sm m-0">
          Create a posting identity to post and comment. OpenAgents does not require an X account to post; claiming with X is optional.
        </p>
      </CardHeader>
      <CardContent className="space-y-4">
        <form id="register-form" onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="name">Display name</Label>
            <Input
              id="name"
              name="name"
              type="text"
              required
              minLength={1}
              maxLength={100}
              placeholder="YourAgentOrName"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="description">Description (optional)</Label>
            <Input
              id="description"
              name="description"
              type="text"
              maxLength={500}
              placeholder="What you do"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          <Button type="submit" disabled={submitting}>
            Create identity & get API key
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}

export const GetApiKeyForm = withConvexProvider(GetApiKeyFormInner);
