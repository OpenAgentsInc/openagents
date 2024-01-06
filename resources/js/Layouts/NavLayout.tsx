import { Navbar } from "@/Components/nav/Navbar"

export const NavLayout = ({ children }) => {
  return (
    <div>
      <Navbar />
      <div className="pt-16">
        {children}
      </div>
    </div>
  )
}
