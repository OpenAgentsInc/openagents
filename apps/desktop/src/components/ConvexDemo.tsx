import { useState } from "react";
import { useQuery, useMutation } from "convex/react";
import { api } from "../convex/_generated/api";

export function ConvexDemo() {
  const messages = useQuery(api.messages.getMessages) || [];
  const messageCount = useQuery(api.messages.getMessageCount) || 0;
  const addMessage = useMutation(api.messages.addMessage);
  
  const [newMessage, setNewMessage] = useState("");
  const [userName] = useState(() => `User-${Math.floor(Math.random() * 1000)}`);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newMessage.trim()) return;
    
    await addMessage({
      body: newMessage.trim(),
      user: userName
    });
    setNewMessage("");
  };

  return (
    <div className="p-4 bg-gray-900 text-white rounded-lg max-w-md">
      <h3 className="text-lg font-bold mb-4">Convex Demo</h3>
      
      <div className="mb-4">
        <p className="text-sm text-gray-400">
          Connected as: <span className="text-green-400">{userName}</span>
        </p>
        <p className="text-sm text-gray-400">
          Total messages: <span className="text-blue-400">{messageCount}</span>
        </p>
      </div>

      <div className="mb-4 max-h-32 overflow-y-auto bg-gray-800 p-2 rounded">
        {messages.length === 0 ? (
          <p className="text-gray-500 text-sm">No messages yet...</p>
        ) : (
          messages.slice(-5).map((msg: any) => (
            <div key={msg._id} className="mb-2 text-sm">
              <span className="text-cyan-400">{msg.user}:</span>{" "}
              <span className="text-gray-200">{msg.body}</span>
              <div className="text-xs text-gray-500">
                {new Date(msg.timestamp).toLocaleTimeString()}
              </div>
            </div>
          ))
        )}
      </div>

      <form onSubmit={handleSubmit} className="space-y-2">
        <input
          type="text"
          value={newMessage}
          onChange={(e) => setNewMessage(e.target.value)}
          placeholder="Type a message..."
          className="w-full px-3 py-2 bg-gray-800 text-white rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
        />
        <button
          type="submit"
          disabled={!newMessage.trim()}
          className="w-full px-3 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white rounded font-medium"
        >
          Send Message
        </button>
      </form>
    </div>
  );
}