import { Navbar } from "@/Components/nav/Navbar"
import { usePage } from "@inertiajs/react"

export const NavLayout = ({ children, noPadding }: any) => {
  const { auth } = usePage().props as any
  return (
    <>
      <Navbar user={auth?.user} />
      <div className={`h-screen ${noPadding ? "" : "pt-16"}`}>
        {children}
      </div>
    </>
  )
}
