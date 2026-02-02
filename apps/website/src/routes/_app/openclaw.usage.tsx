import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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
    <div className="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-6 px-4 py-8">
      <div className="flex flex-col gap-3">
        <Badge variant="outline">Usage</Badge>
        <div className="space-y-1">
          <h1 className="text-3xl font-semibold tracking-tight">Runtime usage</h1>
          <p className="text-sm text-muted-foreground">
            Monitor runtime health, trigger backups, and restart the gateway.
          </p>
        </div>
      </div>

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Gateway status</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2 text-sm">
          {loading ? (
            <div className="text-muted-foreground">Loading status…</div>
          ) : (
            <>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Gateway</span>
                <span className="font-medium capitalize">
                  {status?.gateway.status ?? "unknown"}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Last backup</span>
                <span className="font-medium">
                  {status?.lastBackup
                    ? new Date(status.lastBackup).toLocaleString()
                    : "—"}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Version</span>
                <span className="font-medium">{status?.version.clawdbot ?? "—"}</span>
              </div>
            </>
          )}
        </CardContent>
      </Card>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Backup now</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3 text-sm">
            <p className="text-muted-foreground">
              Trigger an immediate sync of gateway state to R2.
            </p>
            <Button onClick={handleBackup} disabled={working === "backup"}>
              {working === "backup" ? "Backing up…" : "Run backup"}
            </Button>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Restart gateway</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3 text-sm">
            <p className="text-muted-foreground">
              Restart the OpenClaw gateway process without reprovisioning.
            </p>
            <Button variant="secondary" onClick={handleRestart} disabled={working === "restart"}>
              {working === "restart" ? "Restarting…" : "Restart gateway"}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
