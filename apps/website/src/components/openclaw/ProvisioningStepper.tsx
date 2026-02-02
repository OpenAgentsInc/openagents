import { cn } from "@/lib/utils";

const STEPS = ["Creating", "Deploying", "Booting", "Ready"];

function stepIndex(status?: string) {
  if (status === "ready") return STEPS.length - 1;
  if (status === "provisioning") return 1;
  return 0;
}

export function ProvisioningStepper({ status }: { status?: string }) {
  const activeIndex = stepIndex(status);

  return (
    <div className="space-y-2">
      <div className="text-sm font-medium">Provisioning progress</div>
      <div className="grid gap-2 sm:grid-cols-4">
        {STEPS.map((step, index) => (
          <div
            key={step}
            className={cn(
              "rounded-md border px-3 py-2 text-xs",
              index <= activeIndex
                ? "border-primary/40 bg-primary/10 text-primary"
                : "border-border text-muted-foreground",
            )}
          >
            {step}
          </div>
        ))}
      </div>
    </div>
  );
}
