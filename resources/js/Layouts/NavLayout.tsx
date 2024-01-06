import { Navbar } from "@/Components/nav/Navbar"

export const NavLayout = ({ children }) => {
  return (
    <>
      <Navbar />
      <div className="pt-16 h-screen">
        {children}
      </div>
    </>
  )
}
