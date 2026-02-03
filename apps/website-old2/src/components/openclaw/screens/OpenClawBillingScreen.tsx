import { Badge } from "@/components/ui/badge";
import { CreditsWidget } from "@/components/openclaw/CreditsWidget";
import type { BillingSummary } from "@/lib/openclawApi";

export type OpenClawBillingScreenProps = {
  summary: BillingSummary | null;
  error: string | null;
};

export function OpenClawBillingScreen({ summary, error }: OpenClawBillingScreenProps) {
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
