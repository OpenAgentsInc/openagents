import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { CreditsWidget } from "@/components/openclaw/CreditsWidget";
import { getBillingSummary, type BillingSummary } from "@/lib/openclawApi";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/openclaw/billing")({
  component: RouteComponent,
  head: () =>
    buildHead({
      title: `OpenClaw Billing | ${SITE_TITLE}`,
      description: "View your OpenClaw credit balance.",
    }),
});

function RouteComponent() {
  const [summary, setSummary] = useState<BillingSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const data = await getBillingSummary();
      setSummary(data ?? null);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to load billing";
      setError(message);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-6 px-4 py-8">
      <div className="flex flex-col gap-3">
        <Badge variant="outline">Billing</Badge>
        <div className="space-y-1">
          <h1 className="text-3xl font-semibold tracking-tight">Credits & billing</h1>
          <p className="text-sm text-muted-foreground">
            Track your credit balance and monitor usage across the month.
          </p>
        </div>
      </div>

      {error ? (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      <CreditsWidget summary={summary} />
    </div>
  );
}
