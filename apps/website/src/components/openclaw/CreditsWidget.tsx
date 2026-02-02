import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { BillingSummary } from "@/lib/openclawApi";
import { roundUsd } from "@/lib/openclawApi";

export function CreditsWidget({
  summary,
}: {
  summary: BillingSummary | null;
}) {
  const balance = summary ? roundUsd(summary.balance_usd) : null;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-lg">Credits</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        <div className="text-sm text-muted-foreground">Remaining balance</div>
        <div className="text-3xl font-semibold">
          {balance === null ? "—" : `$${balance.toFixed(2)}`}
        </div>
        <div className="text-xs text-muted-foreground">
          Credits are billed in USD. Usage updates every few minutes.
        </div>
      </CardContent>
    </Card>
  );
}
