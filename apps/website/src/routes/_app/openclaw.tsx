import { createFileRoute, Link } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { InstanceStatusCard } from "@/components/openclaw/InstanceStatusCard";
import { ProvisioningStepper } from "@/components/openclaw/ProvisioningStepper";
import {
  createOpenclawInstance,
  getOpenclawInstance,
  getRuntimeStatus,
  type InstanceSummary,
  type RuntimeStatusData,
} from "@/lib/openclawApi";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/openclaw")({
  component: RouteComponent,
  head: () =>
    buildHead({
      title: `OpenClaw | ${SITE_TITLE}`,
      description: "Manage your managed OpenClaw runtime.",
    }),
});

function RouteComponent() {
  const [instance, setInstance] = useState<InstanceSummary | null>(null);
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatusData | null>(null);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [instanceData, statusData] = await Promise.all([
        getOpenclawInstance(),
        getRuntimeStatus().catch(() => null),
      ]);
      setInstance(instanceData ?? null);
      setRuntimeStatus(statusData ?? null);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unable to load OpenClaw";
      setError(message);
    } finally {
      setLoading(false);
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

  const gatewayStatus = runtimeStatus?.gateway.status ?? "unknown";

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-1 flex-col gap-6 px-4 py-8">
      <div className="flex flex-col gap-3">
        <Badge variant="outline">Managed OpenClaw</Badge>
        <div className="space-y-1">
          <h1 className="text-3xl font-semibold tracking-tight">OpenClaw Control</h1>
          <p className="text-sm text-muted-foreground">
            Provision, monitor, and administer your OpenClaw runtime directly from OpenAgents.
          </p>
        </div>
      </div>

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(260px,320px)]">
        <div className="space-y-6">
          <InstanceStatusCard instance={instance} isCreating={creating} onCreate={handleCreate} />

          {instance?.status === "provisioning" ? (
            <ProvisioningStepper status={instance.status} />
          ) : null}

          <Card>
            <CardHeader>
              <CardTitle className="text-lg">Runtime status</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm">
              {loading ? (
                <div className="text-muted-foreground">Loading runtime status…</div>
              ) : (
                <>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">Gateway</span>
                    <span className="font-medium capitalize">{gatewayStatus}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">Last backup</span>
                    <span className="font-medium">
                      {runtimeStatus?.lastBackup
                        ? new Date(runtimeStatus.lastBackup).toLocaleString()
                        : "—"}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">Instance type</span>
                    <span className="font-medium">
                      {runtimeStatus?.container.instanceType ?? "—"}
                    </span>
                  </div>
                </>
              )}
            </CardContent>
          </Card>
        </div>

        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-lg">Quick actions</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <Button asChild variant="secondary" className="w-full">
                <Link to="/openclaw/create">Provision settings</Link>
              </Button>
              <Button asChild variant="secondary" className="w-full">
                <Link to="/openclaw/security">Pair devices</Link>
              </Button>
              <Button asChild variant="secondary" className="w-full">
                <Link to="/openclaw/usage">Usage & backups</Link>
              </Button>
              <Button asChild variant="secondary" className="w-full">
                <Link to="/openclaw/billing">Billing</Link>
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
