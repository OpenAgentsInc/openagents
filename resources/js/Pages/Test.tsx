import DotPattern from "@/Components/magicui/dot-pattern";
import RetroGrid from "@/Components/magicui/retro-grid";
import ShimmerButton from "@/Components/magicui/shimmer-button";
import { cn } from "@/lib/utils";
import { Head } from "@inertiajs/react";

export default function Test() {
  return (
    <>
      <Head title="Home" />
      <div className="dark w-[700px] mx-auto mt-12">
        <div className="relative flex h-full w-full items-center justify-center overflow-hidden rounded-lg border bg-background p-20 shadow-2xl">
          <span className="pointer-events-none z-10 whitespace-pre-wrap bg-gradient-to-b from-[#FFFFFF] via-[#D3D3D3] to-[#A9A9A9] bg-clip-text text-center text-7xl font-bold leading-relaxed text-transparent">
            Open Agents
          </span>
          {/* <span className="pointer-events-none z-10 whitespace-pre-wrap bg-gradient-to-b from-[#666666] via-[#4d4d4d] to-[#000000] bg-clip-text text-center text-7xl font-bold leading-relaxed text-transparent">
            Open Agents
          </span> */}

          <RetroGrid />
        </div>

        <div className="mt-16 relative flex h-full w-full items-center justify-center overflow-hidden rounded-lg border bg-background p-20 shadow-2xl">
          <ShimmerButton className="mx-auto shadow-2xl">
            <span className="whitespace-pre-wrap text-center text-sm font-medium leading-none tracking-tight text-white dark:from-white dark:to-slate-900/10 lg:text-lg">
              Try the beta
            </span>
          </ShimmerButton>
          <DotPattern
            width={20}
            height={20}
            cx={1}
            cy={1}
            cr={1}
            className={cn(
              "[mask-image:linear-gradient(to_bottom_right,white,transparent,transparent)] ",
            )}
          />
        </div>
      </div>
    </>
  )
}
