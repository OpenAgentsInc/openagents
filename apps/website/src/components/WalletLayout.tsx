import React from "react";

export function WalletLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-screen flex-col">
      <header className="flex shrink-0 items-center justify-between border-b border-border bg-background px-4 py-3">
        <a href="/" className="text-sm font-medium text-muted-foreground hover:text-foreground">
          ← Back to site
        </a>
      </header>
      <main className="min-h-0 flex-1">{children}</main>
    </div>
  );
}
