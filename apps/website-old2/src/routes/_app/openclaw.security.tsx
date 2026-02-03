import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { OpenClawSecurityScreen } from "@/components/openclaw/screens/OpenClawSecurityScreen";
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
    <OpenClawSecurityScreen
      devices={devices}
      loading={loading}
      error={error}
      approvingId={approvingId}
      onApprove={handleApprove}
    />
  );
}
