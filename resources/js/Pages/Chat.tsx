import ChatLayout from "@/Layouts/ChatLayout"

function Chat() {
  return (
    <div className="flex flex-row w-screen h-full">
      <div className="hidden w-22 flex-col items-center border-r border-neutral-300 p-3 pt-5 lg:flex">
        <a className="cursor-pointer mb-1 flex h-16 w-16 flex-col items-center justify-center rounded-xl text-neutral-900 hover:bg-neutral-200 hover:text-neutral-900">
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="currentColor"><path d="M12 12C10.9 12 9.95833 11.6083 9.175 10.825C8.39167 10.0417 8 9.1 8 8C8 6.9 8.39167 5.95833 9.175 5.175C9.95833 4.39167 10.9 4 12 4C13.1 4 14.0417 4.39167 14.825 5.175C15.6083 5.95833 16 6.9 16 8C16 9.1 15.6083 10.0417 14.825 10.825C14.0417 11.6083 13.1 12 12 12ZM18 20H6C5.45 20 4.97933 19.8043 4.588 19.413C4.196 19.021 4 18.55 4 18V17.2C4 16.6333 4.146 16.1123 4.438 15.637C4.72933 15.1623 5.11667 14.8 5.6 14.55C6.63333 14.0333 7.68333 13.6457 8.75 13.387C9.81667 13.129 10.9 13 12 13C13.1 13 14.1833 13.129 15.25 13.387C16.3167 13.6457 17.3667 14.0333 18.4 14.55C18.8833 14.8 19.2707 15.1623 19.562 15.637C19.854 16.1123 20 16.6333 20 17.2V18C20 18.55 19.8043 19.021 19.413 19.413C19.021 19.8043 18.55 20 18 20Z"></path></svg>
          <div className="t-label mt-2">Profile</div>
        </a>
      </div >
      <div className="relative grow overflow-x-auto flex flex-col">
        <div className="relative flex flex-col overflow-hidden sm:overflow-x-visible h-full grow">
          <div className="relative grow overflow-y-hidden">
            <div className="h-full">
              <div className="scrollbar-gutter-both-edges relative h-full overflow-y-auto overflow-x-hidden">
                <div className="t-body-chat relative h-full space-y-6 px-5 text-primary-700 w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]">
                  <div className="relative h-8 shrink-0 2xl:h-12 z-30"></div>
                  <div className="pb-6 lg:pb-8 min-h-[calc(100%-60px)] sm:min-h-[calc(100%-120px)]">
                    <div className="relative space-y-6">
                      <div className="space-y-6">

                        <div className="flex justify-end break-anywhere relative py-1">
                          <div className="max-w-[83%] space-y-1 whitespace-pre-wrap">
                            <div className="rounded-[10px] bg-neutral-200 p-3 ml-auto w-fit max-w-full">
                              Who the hell are you?
                            </div>
                          </div>
                        </div>

                        <div className="break-anywhere relative py-1">
                          <div className="flex items-center">
                            <div className="w-full">
                              {[...Array(3)].map((_, i) => (
                                <div key={i} className="whitespace-pre-wrap mb-4 last:mb-0">
                                  <span>Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.</span>
                                </div>
                              ))}
                            </div>
                          </div>
                        </div>

                        <div className="flex justify-end break-anywhere relative py-1">
                          <div className="max-w-[83%] space-y-1 whitespace-pre-wrap">
                            <div className="rounded-[10px] bg-neutral-200 p-3 ml-auto w-fit max-w-full">
                              What is that?
                            </div>
                          </div>
                        </div>

                        <div className="break-anywhere relative py-1">
                          <div className="flex items-center">
                            <div className="w-full">
                              {[...Array(3)].map((_, i) => (
                                <div key={i} className="whitespace-pre-wrap mb-4 last:mb-0">
                                  <span>Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.</span>
                                </div>
                              ))}
                            </div>
                          </div>
                        </div>

                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
          <div className="max-h-[40%] px-5 sm:px-0 z-15 w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]">
            <div className="shadow-lg relative flex h-full w-full cursor-text items-end border border-transparent bg-neutral-25 transition-all duration-300 focus-within:border-neutral-400 focus-within:shadow-none hover:border-neutral-400 hover:shadow-none rounded-[30px]">
              <div className="relative my-1.5 ml-1.5 z-10">
                <button type="button" className="grid h-10 w-12 place-items-center rounded-full transition-colors duration-300 bg-neutral-200 hover:bg-neutral-200-hover active:bg-neutral-200-tap">

                </button>
              </div>
              <div className="h-full grow overflow-y-auto py-3 pr-4 lg:py-[5px] 2xl:py-[8.5px] pl-2">
                <textarea role="textbox" className="t-body-chat block w-full resize-none overflow-y-hidden whitespace-pre-wrap bg-transparent text-primary-700 outline-none placeholder:text-neutral-600" spellCheck="false" placeholder="Say something..." style={{ height: 32 }}></textarea>
              </div>
              <button aria-label="Submit text" className="flex h-9 w-9 items-center justify-center rounded-full p-1.5 text-neutral-600 bg-neutral-50 m-2 transition-colors duration-300" type="button" disabled={false}><svg xmlns="http://www.w3.org/2000/svg" width="13" height="16" fill="currentColor"><path fillRule="evenodd" d="M.852 7.648a1.2 1.2 0 0 1 0-1.696l4.8-4.8a1.2 1.2 0 0 1 1.696 0l4.8 4.8a1.2 1.2 0 1 1-1.697 1.696L7.7 4.897V14a1.2 1.2 0 0 1-2.4 0V4.897L2.548 7.648a1.2 1.2 0 0 1-1.696 0Z" clipRule="evenodd"></path></svg></button>
            </div>
          </div>
          <div className="px-5 py-4 w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]">

          </div>
        </div>
      </div>
    </div>
  )
}

Chat.layout = (page) => <ChatLayout children={page} />

export default Chat
