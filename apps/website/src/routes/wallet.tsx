import { createFileRoute } from "@tanstack/react-router";
import WalletApp from "@/components/wallet/WalletApp";
import { WalletLayout } from "@/components/WalletLayout";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/wallet")({
  component: RouteComponent,
  ssr: false,
  head: () =>
    buildHead({
      title: `Wallet | ${SITE_TITLE}`,
      description: "Self-custodial Lightning wallet powered by Breez SDK.",
    }),
});

function RouteComponent() {
  return (
    <WalletLayout>
      <div className="wallet-page min-h-full bg-background text-foreground">
        <WalletApp />
      </div>
    </WalletLayout>
  );
}
