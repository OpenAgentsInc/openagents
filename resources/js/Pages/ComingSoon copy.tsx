import DotPattern from "@/Components/magicui/dot-pattern";
import ShimmerButton from "@/Components/magicui/shimmer-button";
import InspectLayout from "@/Layouts/InspectLayout";
import { cn } from "@/lib/utils";
import {  Link } from "@inertiajs/react";

function ComingSoon() {
    return (
        <div className="w-[500px] max-w-full px-12 md:w-[700px] mx-auto h-[90vh] flex flex-col justify-center items-center">
            <div className="relative flex w-full items-center justify-center overflow-hidden rounded-lg border bg-background p-6 md:p-20 shadow-2xl">
                <span className="py-1 pointer-events-none z-10 whitespace-pre-wrap bg-gradient-to-b from-[#A9A9A9] via-[#696969] to-[#000000] bg-clip-text text-center text-4xl md:text-5xl font-bold leading-relaxed tracking-wide text-transparent">
                    OpenAgents
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
            <Link href="/inspect">
                <ShimmerButton className="mt-8 mx-auto p-4 md:mt-12 shadow-2xl">
                    <span className="whitespace-pre-wrap text-center text-md md:text-sm font-medium leading-none tracking-tight text-white dark:from-white dark:to-slate-900/10 lg:text-lg">
                        Coming soon
                    </span>
                </ShimmerButton>
            </Link>
        </div>
    )
}

ComingSoon.layout = (page) => <InspectLayout children={page} title="Home" />

export default ComingSoon
