import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { InstanceStatusCard } from "@/components/openclaw/InstanceStatusCard";
import { ProvisioningStepper } from "@/components/openclaw/ProvisioningStepper";
import {
  createOpenclawInstance,
  getOpenclawInstance,
  type InstanceSummary,
} from "@/lib/openclawApi";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/openclaw/create")({
  component: RouteComponent,
  head: () =>
    buildHead({
      title: `Create OpenClaw | ${SITE_TITLE}`,
      description: "Provision a managed OpenClaw instance.",
    }),
});

function RouteComponent() {
  const [instance, setInstance] = useState<InstanceSummary | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const instanceData = await getOpenclawInstance();
      setInstance(instanceData ?? null);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unable to load instance";
      setError(message);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleCreate = useCallback(async () => {
    setCreating(true);
    setError(null);
    try {
      const created = await createOpenclawInstance();
      setInstance(created ?? null);
      await load();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to create instance";
      setError(message);
    } finally {
      setCreating(false);
    }
  }, [load]);

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-6 px-4 py-8">
      <div className="flex flex-col gap-3">
        <Badge variant="outline">Provisioning</Badge>
        <div className="space-y-1">
          <h1 className="text-3xl font-semibold tracking-tight">Create OpenClaw instance</h1>
          <p className="text-sm text-muted-foreground">
            Provision a dedicated runtime with its own sandbox, storage, and service token.
          </p>
        </div>
      </div>

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <InstanceStatusCard instance={instance} isCreating={creating} onCreate={handleCreate} />
      {instance?.status === "provisioning" ? (
        <ProvisioningStepper status={instance.status} />
      ) : null}
    </div>
  );
}
