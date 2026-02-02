import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { OpenClawBillingScreen } from "@/components/openclaw/screens/OpenClawBillingScreen";
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

  return <OpenClawBillingScreen summary={summary} error={error} />;
}
