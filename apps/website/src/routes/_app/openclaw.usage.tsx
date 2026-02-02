import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { OpenClawUsageScreen } from "@/components/openclaw/screens/OpenClawUsageScreen";
import {
  backupRuntime,
  getRuntimeStatus,
  restartRuntime,
  type RuntimeStatusData,
} from "@/lib/openclawApi";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/openclaw/usage")({
  component: RouteComponent,
  head: () =>
    buildHead({
      title: `OpenClaw Usage | ${SITE_TITLE}`,
      description: "Monitor runtime health and trigger backups or restarts.",
    }),
});

function RouteComponent() {
  const [status, setStatus] = useState<RuntimeStatusData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState<"backup" | "restart" | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getRuntimeStatus();
      setStatus(data ?? null);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to load status";
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleBackup = useCallback(async () => {
    setWorking("backup");
    setError(null);
    try {
      await backupRuntime();
      await load();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Backup failed";
      setError(message);
    } finally {
      setWorking(null);
    }
  }, [load]);

  const handleRestart = useCallback(async () => {
    setWorking("restart");
    setError(null);
    try {
      await restartRuntime();
      await load();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Restart failed";
      setError(message);
    } finally {
      setWorking(null);
    }
  }, [load]);

  return (
    <OpenClawUsageScreen
      status={status}
      loading={loading}
      error={error}
      working={working}
      onBackup={handleBackup}
      onRestart={handleRestart}
    />
  );
}
