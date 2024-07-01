import { useEffect, useRef } from "react";

function useChatScroll<T>(dep: T): React.RefObject<HTMLDivElement> {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const scrollToBottom = () => {
      if (ref.current) {
        const { scrollHeight, scrollTop, clientHeight } = ref.current;
        const isAtBottom = scrollHeight - scrollTop - clientHeight < 180;

        if (isAtBottom) {
          ref.current.scrollTop = ref.current.scrollHeight;
        }
      }
    };

    scrollToBottom();

    // Optional: Scroll to bottom after images load
    const images = ref.current?.getElementsByTagName("img");
    if (images) {
      Array.from(images).forEach((img) => {
        img.addEventListener("load", scrollToBottom);
      });
    }

    return () => {
      if (images) {
        Array.from(images).forEach((img) => {
          img.removeEventListener("load", scrollToBottom);
        });
      }
    };
  }, [dep]);

  return ref;
}

export default useChatScroll;
