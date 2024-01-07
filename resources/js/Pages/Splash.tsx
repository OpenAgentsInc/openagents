import { NavLayout } from "@/Layouts/NavLayout";
import { Button } from "@/Components/ui/button";
import { Link } from "@inertiajs/react";
import { KamdoStage } from "@/Components/three";

function Splash() {
  return (
    <>
      <div className="px-4 pointer-events-none absolute top-0 left-0 w-full h-full flex flex-col justify-center items-center" style={{ zIndex: 9999 }}>
        <h1 className="-mt-16 text-7xl font-black uppercase tracking-tight">Make AI <span className="">cool</span> again</h1>
        <h3 className="mt-6 text-2xl font-light tracking-wide">OpenAgents is the community platform for building AI agents.</h3>
        {/* <Link href="/chat">
          <Button size="lg" className="cursor-pointer mt-8 pointer-events-auto" style={{ backgroundColor: "rgba(0,0,0,0.75)" }}>Meet the Concierge agent</Button>
        </Link> */}
      </div>
      <KamdoStage />
    </>
  )
}

Splash.layout = (page) => <NavLayout children={page} noPadding />

export default Splash
