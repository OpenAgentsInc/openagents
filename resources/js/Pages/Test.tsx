import RetroGrid from "@/Components/magicui/retro-grid";
import { Head } from "@inertiajs/react";

export default function Test() {
  return (
    <>
      <Head title="Home" />
      <div className="w-[700px] mx-auto mt-12">
        <div className="relative flex h-full w-full items-center justify-center overflow-hidden rounded-lg border bg-background p-20 shadow-2xl">
          <span className="pointer-events-none z-10 whitespace-pre-wrap bg-gradient-to-b from-[#666666] via-[#4d4d4d] to-[#000000] bg-clip-text text-center text-7xl font-bold leading-relaxed text-transparent">
            Open Agents
          </span>

          <RetroGrid />
        </div>
      </div>
    </>
  )
}
