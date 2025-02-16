import { Send } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

function ChatScreen() {
  return (
    <div className="fixed inset-0 dark bg-black flex flex-col">
      <div className="flex-1 overflow-auto p-4">
        {/* Messages will go here */}
        <div className="space-y-4">
          <div className="bg-gray-800 rounded-lg p-4 max-w-[80%]">
            <p className="text-white">Hello! How can I help you today?</p>
          </div>
        </div>
      </div>

      <div className="p-4 border-t border-gray-800">
        <Card className="w-full">
          <CardContent className="p-2">
            <div className="flex gap-2">
              <input
                type="text"
                placeholder="Type your message..."
                className="flex-1 bg-transparent border-0 focus:outline-none text-white"
              />
              <Button size="sm">
                <Send className="h-4 w-4" />
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

export default ChatScreen;
