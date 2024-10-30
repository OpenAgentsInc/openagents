import { Loader2 } from "lucide-react"
import { useEffect } from "react"
import { ChatInput } from "@/components/chat/ChatInput"
import { ChatList } from "@/components/chat/ChatList"
import { Message } from "@/components/chat/types"
import MainLayout from "@/Layouts/MainLayout"
import { useChat } from "@/lib/useChat"
import { PageProps } from "@/types"
import { Head, useRemember } from "@inertiajs/react"

interface Chat {
  id: number;
  title: string;
  last_message_at: string;
}

export default function CRM({ auth, messages: initialMessages = [], chats, currentChatId = 1 }: PageProps<{ messages: Message[], chats: Chat[], currentChatId: number | null }>) {

  return (
    <MainLayout>
      <Head title="CRM" />
      <div className="relative h-full w-full">
        lollll lets do it
      </div>
    </MainLayout>
  )
}
