// import { Button } from "@/Components/catalyst/button"
import { Button } from "@/Components/ui/button"
import ChatLayout from "@/Layouts/ChatLayout"

function Chat() {
  return (
    <div className="flex flex-row w-screen h-full">
      <div className="w-[5.5rem] border-r border-teal-800/25 shadow-xl nice-scrollbar overflow-y-auto">
        <div>
          {/* <Button>New conversation</Button> */}
        </div>
        <p>ChatList</p>
      </div >
      <div className="relative grow overflow-x-auto flex flex-col">
        <div className="relative flex flex-col overflow-hidden sm:overflow-x-visible h-full pt-8 grow">
          <div className="relative w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]">
            <div className="absolute w-full bg-gradient-to-b from-neutral-50 to-transparent lg:h-[50px] lg:bg-gradient-to-b lg:from-neutral-50 lg:to-transparent z-10 h-[50px]"></div>
          </div>
          <div className="relative grow overflow-y-hidden">
            <div className="h-full">
              <div className="scrollbar-gutter-both-edges relative h-full overflow-y-auto overflow-x-hidden">
                <div className="t-body-chat relative h-full space-y-6 px-5 text-primary-700 w-full mx-auto max-w-1.5xl 2xl:max-w-[47rem]">
                  <div className="relative h-8 shrink-0 2xl:h-12 z-30"></div>
                  <div className="pb-6 lg:pb-8 min-h-[calc(100%-60px)] sm:min-h-[calc(100%-120px)]">
                    <div className="relative space-y-6">
                      <div className="space-y-6">
                        <div className="break-anywhere relative py-1">
                          <div className="flex items-center">
                            <div className="w-full">
                              <div className="whitespace-pre-wrap mb-4 last:mb-0">
                                <span>Hello world</span>
                              </div>
                              <div className="whitespace-pre-wrap mb-4 last:mb-0">
                                <span>Hello world</span>
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
          </div>
        </div>
      </div>
    </div>
  )
}

Chat.layout = (page) => <ChatLayout children={page} />

export default Chat
