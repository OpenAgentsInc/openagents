import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { RuntimeStatusData } from "@/lib/openclawApi";

export type OpenClawUsageScreenProps = {
  status: RuntimeStatusData | null;
  loading: boolean;
  error: string | null;
  working: "backup" | "restart" | null;
  onBackup?: () => void;
  onRestart?: () => void;
};

export function OpenClawUsageScreen({
  status,
  loading,
  error,
  working,
  onBackup,
  onRestart,
}: OpenClawUsageScreenProps) {
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
            <Button onClick={onBackup} disabled={working === "backup"}>
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
            <Button variant="secondary" onClick={onRestart} disabled={working === "restart"}>
              {working === "restart" ? "Restarting…" : "Restart gateway"}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
