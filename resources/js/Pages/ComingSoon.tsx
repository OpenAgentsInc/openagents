import DotPattern from "@/Components/magicui/dot-pattern";
import ShimmerButton from "@/Components/magicui/shimmer-button";
import { cn } from "@/lib/utils";
import { Head, Link } from "@inertiajs/react";

export default function ComingSoon() {
    return (
        <>
            <Head title="Home" />
            <div className="dark w-[500px] max-w-full px-12 md:w-[700px] mx-auto h-screen flex flex-col justify-center items-center">
                <div className="relative flex w-full items-center justify-center overflow-hidden rounded-lg border bg-background p-6 md:p-20 shadow-2xl">
                    <span className="py-1 pointer-events-none z-10 whitespace-pre-wrap bg-gradient-to-b from-[#FFFFFF] via-[#D3D3D3] to-[#A9A9A9] bg-clip-text text-center text-4xl md:text-7xl font-bold leading-relaxed text-transparent">
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
                <a href="/inspect">
                    <ShimmerButton className="mt-8 mx-auto p-4 md:mt-12 shadow-2xl">
                        <span className="whitespace-pre-wrap text-center text-md md:text-sm font-medium leading-none tracking-tight text-white dark:from-white dark:to-slate-900/10 lg:text-lg">
                            Coming soon
                        </span>
                    </ShimmerButton>
                </a>
            </div>
        </>
    )
}
