import DotPattern from "@/Components/magicui/dot-pattern";
import RetroGrid from "@/Components/magicui/retro-grid";
import ShimmerButton from "@/Components/magicui/shimmer-button";
import { cn } from "@/lib/utils";
import { Head } from "@inertiajs/react";

export default function ComingSoon() {
  return (
    <>
      <Head title="Home" />
      <div className="dark w-[700px] mx-auto h-screen flex flex-col justify-center items-center">
        <div className="relative flex w-full items-center justify-center overflow-hidden rounded-lg border bg-background p-20 shadow-2xl">
          <span className="pointer-events-none z-10 whitespace-pre-wrap bg-gradient-to-b from-[#FFFFFF] via-[#D3D3D3] to-[#A9A9A9] bg-clip-text text-center text-7xl font-bold leading-relaxed text-transparent">
            Open Agents
          </span>

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
        <ShimmerButton className="mt-16 mx-auto shadow-2xl">
          <span className="whitespace-pre-wrap text-center text-sm font-medium leading-none tracking-tight text-white dark:from-white dark:to-slate-900/10 lg:text-lg">
            Coming soon
          </span>
        </ShimmerButton>
      </div>
    </>
  )
}
