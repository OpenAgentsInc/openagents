import { ChatComponents } from "../components/library/chat"
import { ShadComponents } from "../components/library/shad"
import { ScrollArea } from "../components/ui/scroll-area"
import { Separator } from "../components/ui/separator"

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
        <h2 className="text-2xl font-semibold mb-6">Chat Components</h2>
        <ChatComponents />
        <div className="my-8"></div>
        <Separator className="my-4" />
        <div className="my-8"></div>
        <h2 className="text-2xl font-semibold mb-6">Shadcn Components</h2>
        <ShadComponents />
      </div>
    </ScrollArea>
  );
}
