import { useState } from 'react'
import { Dialog } from '@headlessui/react'
import { Bars3Icon, XMarkIcon } from '@heroicons/react/24/outline'
import { Link, usePage } from '@inertiajs/react'
import ApplicationLogo from './ApplicationLogo'
import { Button } from './ui/button'
import { router } from "@inertiajs/react"

const navigation = [
  // { name: 'Inspect', href: '/inspect' },
  //   { name: 'Marketplace', href: '/marketplace' },
]

export const Header = () => {
  const props = usePage().props
  const ziggy = props.ziggy as any
  const env = props.env
  const auth = props.auth as any
  const isLoginPage = ziggy.location.includes('login')
  const authed = !!auth.user?.id || false
  const showLogin = true
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)

  return (
    <header className="fixed w-full bg-background/80 shadow backdrop-blur" style={{ zIndex: 9000 }}>
      <nav className="mx-auto flex max-w-7xl items-center justify-between h-16 px-6 lg:px-8" aria-label="Global">
        <Link href="/" className="-m-1.5 p-1.5">
          <ApplicationLogo />
        </Link>
        {showLogin && (
          <div className="flex md:hidden">
            <button
              type="button"
              className="-m-2.5 inline-flex items-center justify-center rounded-md p-2.5 text-gray-700 z-[9999]"
              onClick={() => setMobileMenuOpen(true)}
            >
              <span className="sr-only">Open main menu</span>
              <Bars3Icon className="h-6 w-6" aria-hidden="true" />
            </button>
          </div>
        )}
        <div className="hidden md:flex md:gap-x-12">
          {(showLogin || authed) && <>
            {navigation.map((item: any) => (
              <Link key={item.name} href={item.href} className="text-sm leading-6 text-gray-900">
                {item.name}
              </Link>
            ))}
          </>}

          {showLogin && !authed && !isLoginPage && (
            <Link href="/chat">
              <Button className="text-sm leading-6" variant="outline">
                Sneak peek<span aria-hidden="true" className="pl-2">&rarr;</span>
              </Button>
            </Link>
          )}

          {authed && (
            <Link href="/chat">
              <Button className="text-sm leading-6" variant="outline">
                Chat <span aria-hidden="true" className="pl-2">&rarr;</span>
              </Button>
            </Link>
          )}
        </div>
      </nav>
      {showLogin && (
        <Dialog as="div" className="md:hidden" open={mobileMenuOpen} onClose={setMobileMenuOpen}>
          <div className="fixed inset-0 z-10" />
          <Dialog.Panel className="fixed inset-y-0 right-0 z-10 w-full overflow-y-auto bg-white px-6 py-6 sm:max-w-sm sm:ring-1 sm:ring-gray-900/10">
            <div className="pt-16 flex items-center justify-between">
              <button
                type="button"
                className="-m-2.5 rounded-md p-2.5 text-gray-700"
                onClick={() => setMobileMenuOpen(false)}
              >
                <span className="sr-only">Close menu</span>
                <XMarkIcon className="h-6 w-6" aria-hidden="true" />
              </button>
            </div>
            <div className="mt-6 flow-root">
              <div className="-my-6 divide-y divide-gray-500/10">
                <div className="space-y-2 py-6">
                  {navigation.map((item: any) => (
                    <a
                      key={item.name}
                      href={item.href}
                      className="-mx-3 block rounded-lg px-3 py-2 text-base  leading-7 text-gray-900 hover:bg-gray-50"
                    >
                      {item.name}
                    </a>
                  ))}
                </div>

                {showLogin && !authed && (
                  <div className="py-6">
                    <a
                      href="/login"
                      className="-mx-3 block rounded-lg px-3 py-2.5 text-base  leading-7 text-gray-900 hover:bg-gray-50"
                    >
                      Log in
                    </a>
                  </div>
                )}

                {authed && (
                  <div className="py-6">
                    <Link
                      href="/dashboard"
                      className="-mx-3 block rounded-lg px-3 py-2.5 text-base  leading-7 text-gray-900 hover:bg-gray-50"
                    >
                      Dashboard
                    </Link>
                  </div>
                )}

              </div>
            </div>
          </Dialog.Panel>
        </Dialog>
      )}

    </header>
  )
}
