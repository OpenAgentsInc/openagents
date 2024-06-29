export default function AutoDev() {
  return (
    <div className="from-[#0a0a0a] to-black text-white font-mono min-h-screen bg-gradient-to-b bg-fixed tracking-tight">
      <div className="flex min-h-screen w-full">
        <nav className="z-20 h-screen max-md:pointer-events-none max-md:fixed"></nav>
        <div className="min-h-full w-full min-w-0 flex-1">
          <div className="flex h-screen w-full flex-col overflow-hidden">
            <div className="sticky top-0 z-10 -mb-6 flex h-14 items-center gap-3 pl-11 pr-2 md:pb-0.5 md:pl-6">
              <div className="from-black via-black to-black/0 absolute inset-0 -bottom-7 z-[-1] bg-gradient-to-b via-50% blur"></div>
              <div className="flex min-w-0 flex-1 shrink flex-col md:flex-row md:items-center 2xl:justify-center">
                <div className="flex min-w-0 items-center max-md:text-sm">
                  <button
                    className="inline-flex items-center justify-center relative shrink-0 ring-offset-2 ring-offset-bg-300 ring-accent-main-100 focus-visible:outline-none focus-visible:ring-1 disabled:pointer-events-none disabled:opacity-50 disabled:shadow-none disabled:drop-shadow-none text-zinc-200 transition-all active:bg-bg-400 hover:bg-bg-500/40 hover:text-text-100 rounded py-1 px-2 max-w-full whitespace-nowrap text-ellipsis overflow-hidden outline-none ring-offset-2 ring-offset-bg-300 ring-accent-main-100 focus-visible:outline-none focus-visible:ring-1 focus:backdrop-blur-xl hover:backdrop-blur-xl hover:bg-bg-400/50 !text-text-000 !shrink gap-1 !px-1 !py-0.5"
                    data-testid="chat-menu-trigger"
                    type="button"
                    id="radix-:r33:"
                    aria-haspopup="menu"
                    aria-expanded="false"
                    data-state="closed"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate font-normal tracking-tight">
                        AutoDev Demo
                      </div>
                    </div>
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="12"
                      height="12"
                      fill="currentColor"
                      viewBox="0 0 256 256"
                    >
                      <path d="M216.49,104.49l-80,80a12,12,0,0,1-17,0l-80-80a12,12,0,0,1,17-17L128,159l71.51-71.52a12,12,0,0,1,17,17Z"></path>
                    </svg>
                  </button>
                </div>
              </div>
            </div>
            <div className="relative flex w-full flex-1 overflow-x-hidden overflow-y-scroll pt-6 md:pr-8">
              <div className="relative mx-auto flex h-full w-full max-w-3xl flex-1 flex-col md:px-2">
                <div className="flex-1  flex  flex-col  gap-3  px-4  max-w-3xl  mx-auto  w-full pt-1">
                  {[...Array(20)].map((_, i) => (
                    <p className="mt-6" key={i}>
                      AutoDev awaiting instructions.
                    </p>
                  ))}
                </div>
                <div className="sticky bottom-0 mx-auto w-full pt-6">
                  <p>Enter your text here</p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
