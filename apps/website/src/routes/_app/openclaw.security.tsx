import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { DeviceList } from "@/components/openclaw/DeviceList";
import {
  approveRuntimeDevice,
  getRuntimeDevices,
  type RuntimeDevicesData,
} from "@/lib/openclawApi";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/openclaw/security")({
  component: RouteComponent,
  head: () =>
    buildHead({
      title: `OpenClaw Security | ${SITE_TITLE}`,
      description: "Approve device pairing requests for OpenClaw.",
    }),
});

function RouteComponent() {
  const [devices, setDevices] = useState<RuntimeDevicesData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [approvingId, setApprovingId] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getRuntimeDevices();
      setDevices(data ?? null);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to load devices";
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleApprove = useCallback(
    async (requestId: string) => {
      setApprovingId(requestId);
      setError(null);
      try {
        await approveRuntimeDevice({ requestId });
        await load();
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to approve device";
        setError(message);
      } finally {
        setApprovingId(null);
      }
    },
    [load],
  );

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

      <DeviceList devices={devices} approvingId={approvingId} onApprove={handleApprove} />
    </div>
  );
}
