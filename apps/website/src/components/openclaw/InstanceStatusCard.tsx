import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { InstanceSummary } from "@/lib/openclawApi";

const statusTone = (status?: string) => {
  switch (status) {
    case "ready":
      return "default";
    case "provisioning":
      return "secondary";
    case "error":
      return "destructive";
    default:
      return "outline";
  }
};

const statusLabel = (status?: string) => {
  switch (status) {
    case "ready":
      return "Ready";
    case "provisioning":
      return "Provisioning";
    case "error":
      return "Error";
    case "deleted":
      return "Deleted";
    default:
      return "Not created";
  }
};

export function InstanceStatusCard({
  instance,
  isCreating,
  onCreate,
}: {
  instance: InstanceSummary | null;
  isCreating?: boolean;
  onCreate?: () => void;
}) {
  const status = instance?.status;
  const updatedAt = instance?.updated_at
    ? new Date(instance.updated_at).toLocaleString()
    : null;

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0">
        <CardTitle className="text-lg">Instance status</CardTitle>
        <Badge variant={statusTone(status)}>{statusLabel(status)}</Badge>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="text-sm text-muted-foreground">
          {instance
            ? "Your OpenClaw runtime is managed by OpenAgents."
            : "Create an OpenClaw runtime to get started."}
        </div>
        {instance?.runtime_name ? (
          <div className="text-sm">
            <span className="text-muted-foreground">Runtime:</span>{" "}
            <span className="font-medium text-foreground">{instance.runtime_name}</span>
          </div>
        ) : null}
        {updatedAt ? (
          <div className="text-xs text-muted-foreground">Last updated: {updatedAt}</div>
        ) : null}
        {!instance && onCreate ? (
          <Button onClick={onCreate} disabled={isCreating}>
            {isCreating ? "Creating…" : "Create OpenClaw instance"}
          </Button>
        ) : null}
      </CardContent>
    </Card>
  );
}
