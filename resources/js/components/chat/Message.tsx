import ReactMarkdown from "react-markdown"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { IconOpenAgents } from "@/components/ui/icons"
import { cn } from "@/lib/utils"
import { usePage } from "@inertiajs/react"
import { ChatMessageActions } from "./ChatMessageActions"
import { ChatMessageProps } from "./types"

function getInitials(name: string | undefined): string {
  if (!name) return "";
  return name
    .split(' ')
    .map(word => word[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);
}

export function Message({ message }: ChatMessageProps) {
  const { auth } = usePage().props;
  const isUser = message.role === 'user';
  const userName = message.user?.name || (isUser ? auth.user.name : undefined);
  const displayInitials = isUser ? getInitials(userName) : "";

  return (
    <div className={cn('group relative mb-4 flex items-start')}>
      <div className="mr-3">
        <Avatar className="rounded-sm">
          <AvatarFallback className={cn(
            'flex size-7 shrink-0 select-none items-center justify-center rounded-sm border border-zinc-800 shadow',
            isUser ? 'bg-black text-white' : 'bg-black text-white'
          )}>
            {isUser ? displayInitials : <IconOpenAgents className="w-4 h-4" />}
          </AvatarFallback>
        </Avatar>
      </div>
      <div className="flex-1 relative">
        {message.content !== "" && message.content.length > 0 && (
          <ReactMarkdown className="prose prose-full-width dark:prose-invert text-sm">{message.content}</ReactMarkdown>
        )}
        <ChatMessageActions message={message} />
      </div>
    </div>
  );
}
