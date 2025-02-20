import { Separator } from "@radix-ui/react-separator"
import { ShadComponents } from "../components/library/shad"
import { ScrollArea } from "../components/ui/scroll-area"

import type { Route } from "../+types/company";
export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Component Library" },
    { name: "description", content: "Component library" },
  ];
}

export default function ComponentLibrary() {
  return (
    <ScrollArea className="h-full">
      <div className="container mx-auto py-8 px-4">
        <h1 className="text-3xl font-bold mb-8">Component Library</h1>
        <ChatComponents />
        <Separator className="my-4" />
        <ShadComponents />
      </div>
    </ScrollArea>
  );
}
