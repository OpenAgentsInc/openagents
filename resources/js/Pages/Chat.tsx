// import { Button } from "@/Components/catalyst/button"
import { Button } from "@/Components/ui/button"
import ChatLayout from "@/Layouts/ChatLayout"

function Chat() {
  return (
    <div className="flex flex-row w-screen h-full">
      <div className="w-1/5 border-r border-teal-800/25 shadow-xl nice-scrollbar overflow-y-auto">
        <div>
          <Button>New conversation</Button>
        </div>
        <p>ChatList</p>
      </div >
      <div className="w-4/5 flex flex-col px-2">
        <div id="chatbox-container" className="grow nice-scrollbar weird-height">
          <p>Chatbox</p>

          {/* <div id="inference-message" className="text-center text-gray-500">
            <div className="flex flex-row justify-center">
              <div className="w-4 h-4 bg-gray-500 rounded-full animate-ping"></div>
              <div className="w-4 h-4 bg-gray-500 rounded-full animate-ping"></div>
              <div className="w-4 h-4 bg-gray-500 rounded-full animate-ping"></div>
            </div>
            <div className="mt-2">
              <span className="text-gray-500">Inference in progress...</span>
              <span id="inferred" className="text-white"></span>
            </div>
          </div> */}
        </div>

        <p>SendMessage</p>

      </div>
    </div>
  )
}

Chat.layout = (page) => <ChatLayout children={page} />

export default Chat
