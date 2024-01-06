import { Features } from "@/Components/landing/Features";
import { Hero } from "@/Components/landing/Hero";
import { Navbar } from "@/Components/nav/Navbar";
import { NavLayout } from "@/Layouts/NavLayout";
import { SidebarLayout } from "@/Layouts/SidebarLayout";

function Splash() {
  return (
    <>
      {/* <Navbar /> */}
      {/* <Hero /> */}
      {/* <Features /> */}
    </>
  )
}

Splash.layout = (page) => <NavLayout children={page} />

export default Splash
