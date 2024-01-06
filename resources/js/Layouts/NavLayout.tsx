import { Navbar } from "@/Components/nav/Navbar"
import { usePage } from "@inertiajs/react"

export const NavLayout = ({ children }: any) => {
  const { auth } = usePage().props as any
  return (
    <>
      <Navbar user={auth?.user} />
      <div className="pt-16 h-screen">
        {children}
      </div>
    </>
  )
}
