import React from "react";
import DragWindowRegion from "@/components/DragWindowRegion";

export default function BaseLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <>
      <DragWindowRegion />
      <main className="text-primary h-screen !font-mono">{children}</main>
    </>
  );
}
