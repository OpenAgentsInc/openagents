import React from "react"
import { ShadComponents } from "../components/shad/components"
import { ScrollArea } from "../components/ui/scroll-area"

export default function ComponentLibrary() {
  return (
    <ScrollArea className="h-full">
      <div className="container mx-auto py-8 px-4">
        <h1 className="text-3xl font-bold mb-8">Component Library</h1>
        <ShadComponents />
      </div>
    </ScrollArea>
  )
}
