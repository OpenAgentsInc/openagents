import { Button } from "@/components/ui/button";
import React from "react";

export default function HomePage() {
  return (
    <div className="font-mono flex flex-col h-full text-white">
      <div className="mx-8 flex flex-row justify-between items-center p-2">
        Coder
        <Button variant="outline">Test</Button>
      </div>
    </div>
  );
}
