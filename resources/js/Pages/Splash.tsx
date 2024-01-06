import { Features } from "@/Components/landing/Features";
import { Hero } from "@/Components/landing/Hero";
import { SidebarLayout } from "@/Layouts/SidebarLayout";

function Splash() {
  return (
    <>
      <Hero />
      <Features />
    </>
  )
}

Splash.layout = (page) => <SidebarLayout children={page} />

export default Splash
