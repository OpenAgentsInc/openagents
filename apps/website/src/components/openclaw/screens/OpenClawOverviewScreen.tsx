import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { InstanceStatusCard } from "@/components/openclaw/InstanceStatusCard";
import { ProvisioningStepper } from "@/components/openclaw/ProvisioningStepper";
import type { InstanceSummary, RuntimeStatusData } from "@/lib/openclawApi";

export type OpenClawOverviewScreenProps = {
  instance: InstanceSummary | null;
  runtimeStatus: RuntimeStatusData | null;
  loading: boolean;
  creating: boolean;
  error: string | null;
  onCreate?: () => void;
  actions?: ReactNode;
};

export function OpenClawOverviewScreen({
  instance,
  runtimeStatus,
  loading,
  creating,
  error,
  onCreate,
  actions,
}: OpenClawOverviewScreenProps) {
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
          <InstanceStatusCard instance={instance} isCreating={creating} onCreate={onCreate} />

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
              {actions ?? (
                <div className="text-sm text-muted-foreground">No actions available.</div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
