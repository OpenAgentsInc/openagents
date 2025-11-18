"use client";

import { useQuery } from "convex/react";
import { api } from "@/convex/_generated/api";

export function EventCount() {
  const count = useQuery(api.acp.countEvents);
  return <span>{count ?? "…"}</span>;
}

