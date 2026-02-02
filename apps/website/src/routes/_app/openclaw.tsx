import { createFileRoute, Link } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { OpenClawOverviewScreen } from "@/components/openclaw/screens/OpenClawOverviewScreen";
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

  const actions = useMemo(
    () => (
      <>
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
      </>
    ),
    [],
  );

  return (
    <OpenClawOverviewScreen
      instance={instance}
      runtimeStatus={runtimeStatus}
      loading={loading}
      creating={creating}
      error={error}
      onCreate={handleCreate}
      actions={actions}
    />
  );
}
