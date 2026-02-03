import { Badge } from "@/components/ui/badge";
import { DeviceList } from "@/components/openclaw/DeviceList";
import type { RuntimeDevicesData } from "@/lib/openclawApi";

export type OpenClawSecurityScreenProps = {
  devices: RuntimeDevicesData | null;
  loading: boolean;
  error: string | null;
  approvingId: string | null;
  onApprove?: (requestId: string) => void;
};

export function OpenClawSecurityScreen({
  devices,
  loading,
  error,
  approvingId,
  onApprove,
}: OpenClawSecurityScreenProps) {
  return (
    <div className="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-6 px-4 py-8">
      <div className="flex flex-col gap-3">
        <Badge variant="outline">Security</Badge>
        <div className="space-y-1">
          <h1 className="text-3xl font-semibold tracking-tight">Device pairing</h1>
          <p className="text-sm text-muted-foreground">
            Approve new devices that want to control your OpenClaw runtime.
          </p>
        </div>
      </div>

      {loading ? (
        <div className="text-sm text-muted-foreground">Loading devices…</div>
      ) : null}
      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <DeviceList devices={devices} approvingId={approvingId} onApprove={onApprove} />
    </div>
  );
}
