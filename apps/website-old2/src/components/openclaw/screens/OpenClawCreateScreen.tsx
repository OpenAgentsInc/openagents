import { Badge } from "@/components/ui/badge";
import { InstanceStatusCard } from "@/components/openclaw/InstanceStatusCard";
import { ProvisioningStepper } from "@/components/openclaw/ProvisioningStepper";
import type { InstanceSummary } from "@/lib/openclawApi";

export type OpenClawCreateScreenProps = {
  instance: InstanceSummary | null;
  creating: boolean;
  error: string | null;
  onCreate?: () => void;
};

export function OpenClawCreateScreen({
  instance,
  creating,
  error,
  onCreate,
}: OpenClawCreateScreenProps) {
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

      <InstanceStatusCard instance={instance} isCreating={creating} onCreate={onCreate} />
      {instance?.status === "provisioning" ? (
        <ProvisioningStepper status={instance.status} />
      ) : null}
    </div>
  );
}
