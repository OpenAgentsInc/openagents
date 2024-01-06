import { Link } from "@inertiajs/react"

export const AgentShowcase = () => {
  return (
    <div className="relative flex h-full max-w-full flex-1 flex-col overflow-hidden">
      <main className="relative h-full w-full flex-1 overflow-auto transition-width">
        <div>
          <div className="mx-auto max-w-3xl px-4 py-12">
            <div className="scroll-mt-28 last:h-[calc(100vh-8rem)]">
              <div tabIndex={0} data-projection-id="2" style={{ opacity: 1, transform: 'none' }}>
                <div className="text-xl font-medium md:text-2xl">My Agents</div>
              </div>
              <div className="mb-10 mt-4">
                <div tabIndex={0} data-projection-id="3" style={{ opacity: 1, transform: 'none' }}>
                  <Link className="flex items-center px-2 py-4 rounded-xl hover:bg-gray-50 dark:hover:bg-gray-700" href="/builder">
                    <div className="flex grow items-center overflow-hidden md:w-3/5 md:grow-0">
                      <div className="flex h-[42px] w-[42px] flex-shrink-0 items-center justify-center rounded-full border border-dashed border-black bg-white dark:border-gray-500 dark:bg-gray-700">
                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-sm">
                          <path d="M12 4L12 20M20 12L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"></path>
                        </svg>
                      </div>
                      <div className="grow overflow-hidden pl-4 pr-9 leading-tight hover:cursor-pointer">
                        <div className="flex items-center gap-1">
                          <span className="font-medium">
                            <div className="flex items-center gap-1">Create an Agent <span className="rounded-full bg-blue-100 px-1.5 py-0.5 text-[10px] font-medium text-blue-600">Beta</span></div>
                          </span>
                        </div>
                        <div className="overflow-hidden text-ellipsis break-words text-sm line-clamp-2">Customize your own agent for a specific purpose</div>
                      </div>
                    </div>
                  </Link>
                </div>
                <div className="h-px bg-gray-100 dark:bg-gray-700"></div>
                <div tabIndex={0} data-projection-id="4" style={{ opacity: 1, transform: 'none' }}>
                  <a className="flex items-center px-2 py-4 rounded-xl hover:bg-gray-50 dark:hover:bg-gray-700" href="/g/g-0dTwriHwS-testo-mang">
                    <div className="flex grow items-center overflow-hidden md:w-3/5 md:grow-0">
                      <div className="h-[42px] w-[42px] flex-shrink-0">
                        <div className="gizmo-shadow-stroke overflow-hidden rounded-full">
                          <img src="https://files.oaiusercontent.com/file-BT1nzPvXlTvhjDnmrqLPHESn?se=2123-12-12T17%3A05%3A50Z&amp;sp=r&amp;sv=2021-08-06&amp;sr=b&amp;rscc=max-age%3D1209600%2C%20immutable&amp;rscd=attachment%3B%20filename%3D61a5df23-ba17-4682-aaaf-12416036e7bb.png&amp;sig=7UMq2y4yq002sDE6Sc33Ok%2BZ0vMJ%2B6i7iMhdnBgbj0A%3D" className="h-full w-full bg-token-surface-secondary dark:bg-token-surface-tertiary" alt="GPT" width="80" height="80" />
                        </div>
                      </div>
                      <div className="grow overflow-hidden pl-4 pr-9 leading-tight hover:cursor-pointer">
                        <div className="flex items-center gap-1">
                          <span className="font-medium">testo mang</span>
                        </div>
                        <div className="overflow-hidden text-ellipsis break-words text-sm line-clamp-2">Friendly and informative expert on OpenAI GPT and Assistant documentation, with a touch of humor.</div>
                        <div className="text-ellipsis text-sm text-gray-500 md:hidden">
                          <div>
                            <div className="flex items-center gap-1">
                              <svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-sm">
                                <circle cx="9.99967" cy="9.99992" r="2.91667" stroke="currentColor" strokeWidth="1.66667"></circle>
                                <path d="M10 3.33325C5.29295 3.33325 2.48502 7.67408 1.57619 9.35031C1.35463 9.75895 1.35463 10.2409 1.57619 10.6495C2.48502 12.3258 5.29295 16.6666 10 16.6666C14.707 16.6666 17.515 12.3258 18.4238 10.6495C18.6454 10.2409 18.6454 9.75895 18.4238 9.35031C17.515 7.67408 14.707 3.33325 10 3.33325Z" stroke="currentColor" strokeWidth="1.66667" strokeLinejoin="round"></path>
                              </svg>
                              <span className="line-clamp-1">Anyone with a link</span>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                    <div className="hidden flex-1 text-ellipsis text-sm text-gray-500 md:block">
                      <div>
                        <div className="flex items-center gap-1">
                          <svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-sm">
                            <circle cx="9.99967" cy="9.99992" r="2.91667" stroke="currentColor" strokeWidth="1.66667"></circle>
                            <path d="M10 3.33325C5.29295 3.33325 2.48502 7.67408 1.57619 9.35031C1.35463 9.75895 1.35463 10.2409 1.57619 10.6495C2.48502 12.3258 5.29295 16.6666 10 16.6666C14.707 16.6666 17.515 12.3258 18.4238 10.6495C18.6454 10.2409 18.6454 9.75895 18.4238 9.35031C17.515 7.67408 14.707 3.33325 10 3.33325Z" stroke="currentColor" strokeWidth="1.66667" strokeLinejoin="round"></path>
                          </svg>
                          <span className="line-clamp-1">Anyone with a link</span>
                        </div>
                      </div>
                    </div>
                    <div className="flex h-9 shrink-0 justify-end gap-2 font-medium md:w-[100px]">
                      <span className="flex justify-center" data-state="closed">
                        <button className="rounded-lg px-3 py-2 text-token-text-primary transition-transform duration-100 ease-in hover:bg-white active:scale-[0.9] dark:bg-transparent dark:hover:bg-gray-700">
                          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-sm">
                            <path fillRule="evenodd" clipRule="evenodd" d="M13.2929 4.29291C15.0641 2.52167 17.9359 2.52167 19.7071 4.2929C21.4783 6.06414 21.4783 8.93588 19.7071 10.7071L18.7073 11.7069L11.1603 19.2539C10.7182 19.696 10.1489 19.989 9.53219 20.0918L4.1644 20.9864C3.84584 21.0395 3.52125 20.9355 3.29289 20.7071C3.06453 20.4788 2.96051 20.1542 3.0136 19.8356L3.90824 14.4678C4.01103 13.8511 4.30396 13.2818 4.7461 12.8397L13.2929 4.29291ZM13 7.41422L6.16031 14.2539C6.01293 14.4013 5.91529 14.591 5.88102 14.7966L5.21655 18.7835L9.20339 18.119C9.40898 18.0847 9.59872 17.9871 9.7461 17.8397L16.5858 11L13 7.41422ZM18 9.5858L14.4142 6.00001L14.7071 5.70712C15.6973 4.71693 17.3027 4.71693 18.2929 5.70712C19.2831 6.69731 19.2831 8.30272 18.2929 9.29291L18 9.5858Z" fill="currentColor"></path>
                          </svg>
                        </button>
                      </span>
                      <button color="neutral" className="rounded-lg bg-transparent px-3 py-2 text-token-text-primary duration-100 ease-in hover:bg-white active:scale-[0.9] dark:bg-transparent dark:hover:bg-gray-700" type="button" id="radix-:re:" aria-haspopup="menu" aria-expanded="false" data-state="closed">
                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-md">
                          <path fillRule="evenodd" clipRule="evenodd" d="M3 12C3 10.8954 3.89543 10 5 10C6.10457 10 7 10.8954 7 12C7 13.1046 6.10457 14 5 14C3.89543 14 3 13.1046 3 12ZM10 12C10 10.8954 10.8954 10 12 10C13.1046 10 14 10.8954 14 12C14 13.1046 13.1046 14 12 14C10.8954 14 10 13.1046 10 12ZM17 12C17 10.8954 17.8954 10 19 10C20.1046 10 21 10.8954 21 12C21 13.1046 20.1046 14 19 14C17.8954 14 17 13.1046 17 12Z" fill="currentColor"></path>
                        </svg>
                      </button>
                    </div>
                  </a>
                </div>
              </div>
            </div>

            <div className="scroll-mt-28 last:h-[calc(100vh-8rem)]">
              <div tabIndex={0} data-projection-id="11" style={{ opacity: 1, transform: 'none' }}><div className="text-xl font-medium md:text-2xl">Made by OpenAgents</div></div>
              <div className="mb-10 mt-4">
                <div tabIndex={0} data-projection-id="12" style={{ opacity: 1, transform: 'none' }}><a className="flex items-center px-2 py-4 rounded-xl hover:bg-gray-50 dark:hover:bg-gray-700" href="/g/g-2fkFE8rbu-dall-e"><div className="flex grow items-center overflow-hidden md:w-3/5 md:grow-0"><div className="h-[42px] w-[42px] flex-shrink-0"><div className="gizmo-shadow-stroke overflow-hidden rounded-full"><img src="https://files.oaiusercontent.com/file-SxYQO0Fq1ZkPagkFtg67DRVb?se=2123-10-12T23%3A57%3A32Z&amp;sp=r&amp;sv=2021-08-06&amp;sr=b&amp;rscc=max-age%3D31536000%2C%20immutable&amp;rscd=attachment%3B%20filename%3Dagent_3.webp&amp;sig=pLlQh8oUktqQzhM09SDDxn5aakqFuM2FAPptuA0mbqc%3D" className="h-full w-full bg-token-surface-secondary dark:bg-token-surface-tertiary" alt="GPT" width="80" height="80" /></div></div><div className="grow overflow-hidden pl-4 pr-9 leading-tight hover:cursor-pointer"><div className="flex items-center gap-1"><span className="font-medium">Concierge</span></div><div className="overflow-hidden text-ellipsis break-words text-sm line-clamp-2">Ask </div><div className="text-ellipsis text-sm text-gray-500 md:hidden"><div className="text-sm text-token-text-tertiary">By OpenAgents</div></div></div></div><div className="hidden flex-1 text-ellipsis text-sm text-gray-500 md:block"><div className="text-sm text-token-text-tertiary">By OpenAgents</div></div><div className="flex h-9 shrink-0 justify-end gap-2 font-medium md:w-[100px]"></div></a></div>
                <div tabIndex={0} data-projection-id="13 " style={{ opacity: 1, transform: 'none' }}><a className="flex items-center px-2 py-4 rounded-xl hover:bg-gray-50 dark:hover:bg-gray-700" href="/g/g-HMNcP6w7d-data-analyst"><div className="flex grow items-center overflow-hidden md:w-3/5 md:grow-0"><div className="h-[42px] w-[42px] flex-shrink-0"><div className="gizmo-shadow-stroke overflow-hidden rounded-full"><img src="https://files.oaiusercontent.com/file-id374Jq85g2WfDgpuOdAMTEk?se=2123-10-13T00%3A31%3A06Z&amp;sp=r&amp;sv=2021-08-06&amp;sr=b&amp;rscc=max-age%3D31536000%2C%20immutable&amp;rscd=attachment%3B%20filename%3Dagent_2.png&amp;sig=qFnFnFDVevdJL3xvtDE8vysDpTQmkSlF1zhYLAMiqmM%3D" className="h-full w-full bg-token-surface-secondary dark:bg-token-surface-tertiary" alt="GPT" width="80" height="80" /></div></div><div className="grow overflow-hidden pl-4 pr-9 leading-tight hover:cursor-pointer"><div className="flex items-center gap-1"><span className="font-medium">AutoDev</span></div><div className="overflow-hidden text-ellipsis break-words text-sm line-clamp-2">Drop in any files and I can help analyze and visualize your data.</div><div className="text-ellipsis text-sm text-gray-500 md:hidden"><div className="text-sm text-token-text-tertiary">By OpenAgents</div></div></div></div><div className="hidden flex-1 text-ellipsis text-sm text-gray-500 md:block"><div className="text-sm text-token-text-tertiary">By OpenAgents</div></div><div className="flex h-9 shrink-0 justify-end gap-2 font-medium md:w-[100px]"></div></a></div>
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>
  )
}
