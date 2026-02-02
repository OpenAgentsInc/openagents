import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { RuntimeDevicesData } from "@/lib/openclawApi";

function formatClient(client?: { platform?: string; mode?: string }) {
  if (!client) return "Unknown client";
  const parts = [client.platform, client.mode].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : "Unknown client";
}

export function DeviceList({
  devices,
  approvingId,
  onApprove,
}: {
  devices: RuntimeDevicesData | null;
  approvingId?: string | null;
  onApprove?: (requestId: string) => void;
}) {
  const pending = devices?.pending ?? [];
  const paired = devices?.paired ?? [];

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-lg">Paired devices</CardTitle>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="space-y-3">
          <div className="text-sm font-medium">Pending approvals</div>
          {pending.length === 0 ? (
            <div className="text-sm text-muted-foreground">No pending devices.</div>
          ) : (
            <div className="space-y-2">
              {pending.map((device) => (
                <div
                  key={device.requestId}
                  className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border px-3 py-2"
                >
                  <div>
                    <div className="text-sm font-medium">{device.requestId}</div>
                    <div className="text-xs text-muted-foreground">
                      {formatClient(device.client)}
                    </div>
                  </div>
                  {onApprove ? (
                    <Button
                      size="sm"
                      onClick={() => onApprove(device.requestId)}
                      disabled={approvingId === device.requestId}
                    >
                      {approvingId === device.requestId ? "Approving…" : "Approve"}
                    </Button>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </div>
        <div className="space-y-3">
          <div className="text-sm font-medium">Paired</div>
          {paired.length === 0 ? (
            <div className="text-sm text-muted-foreground">No paired devices yet.</div>
          ) : (
            <div className="space-y-2">
              {paired.map((device) => (
                <div
                  key={device.deviceId}
                  className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border px-3 py-2"
                >
                  <div>
                    <div className="text-sm font-medium">{device.deviceId}</div>
                    <div className="text-xs text-muted-foreground">
                      {formatClient(device.client)}
                    </div>
                  </div>
                  {device.pairedAt ? (
                    <div className="text-xs text-muted-foreground">
                      Paired {new Date(device.pairedAt).toLocaleString()}
                    </div>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
