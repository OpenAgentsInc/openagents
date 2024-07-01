import React from "react";
import { AnimatedMessage } from "./AnimatedMessage";

interface MessageProps {
  content: string;
  isUser: boolean;
  isComplete: boolean;
  messageId: string;
}

export const Message: React.FC<MessageProps> = ({
  content,
  isUser,
  isComplete,
  messageId,
}) => {
  const isStreaming = !isUser && !isComplete;

  return (
    <div
      className={`
        group relative pt-3.5 pb-[1.125rem] px-4 relative rounded-2xl -tracking-[0.015em]
        bg-[linear-gradient(to_bottom,_hsla(var(--bg-000)/0.75)_0%,_hsla(var(--bg-000)_/_0)_90%)]
        before:absolute before:inset-0
        before:bg-[radial-gradient(ellipse_at_left_top,_hsla(var(--bg-000)/0.5)_0%,_hsla(var(--bg-000)/0.3)_60%)]
        before:rounded-2xl before:border-[0.5px] before:border-[hsla(var(--border-100)/0.15)]
        before:shadow-[0_4px_24px_rgba(0,0,0,0.015)]
        before:[transition:opacity_150ms_ease-out,_transform_250ms_cubic-bezier(0.695,0.555,0.655,1.650)]
        before:z-0 ${isStreaming ? "before:opacity-0 before:scale-[0.995]" : ""}
        ${isUser ? "bg-zinc-900" : "bg-zinc-800"}
      `}
      data-is-streaming={isStreaming.toString()}
    >
      {isStreaming ? (
        <AnimatedMessage content={content} messageId={messageId} />
      ) : (
        <div>{content}</div>
      )}
    </div>
  );
};
