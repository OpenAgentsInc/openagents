import React, { useState, useRef, useEffect } from "react";
import { useTransition, animated, config } from "@react-spring/web";

interface AnimatedMessageProps {
  content: string;
  messageId: string;
}

export function AnimatedMessage({ content, messageId }: AnimatedMessageProps) {
  const [animatedContent, setAnimatedContent] = useState<string>("");
  const queueRef = useRef<string>("");
  const animatingRef = useRef(false);

  useEffect(() => {
    if (content.length > animatedContent.length) {
      queueRef.current = content.slice(animatedContent.length);
      animateNextChar();
    }
  }, [content, animatedContent]);

  const animateNextChar = () => {
    if (animatingRef.current || queueRef.current.length === 0) return;

    animatingRef.current = true;
    const nextChar = queueRef.current[0];

    setAnimatedContent((prev) => prev + nextChar);
    queueRef.current = queueRef.current.slice(1);

    setTimeout(() => {
      animatingRef.current = false;
      animateNextChar();
    }, 5); // Adjust this value to control animation speed
  };

  const tokens = animatedContent.match(/\S+|\s+/g) || [];

  const transitions = useTransition(tokens, {
    keys: (item, index) => `${messageId}-${index}`,
    from: { opacity: 0, transform: "translateY(5px)" },
    enter: { opacity: 1, transform: "translateY(0px)" },
    leave: { opacity: 0 },
    config: config.stiff,
  });

  return (
    <span>
      {transitions((style, item) => (
        <animated.span style={style}>{item}</animated.span>
      ))}
    </span>
  );
}
