import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { OpenClawCreateScreen } from "@/components/openclaw/screens/OpenClawCreateScreen";
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
    <OpenClawCreateScreen
      instance={instance}
      creating={creating}
      error={error}
      onCreate={handleCreate}
    />
  );
}
