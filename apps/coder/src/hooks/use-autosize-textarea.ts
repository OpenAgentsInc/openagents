import { useEffect } from "react"

export interface UseAutosizeTextAreaProps {
  ref: React.RefObject<HTMLTextAreaElement | null>;
  value: string;
}

export function useAutosizeTextArea({ ref, value }: UseAutosizeTextAreaProps) {
  useEffect(() => {
    if (!ref.current) return;

    const textarea = ref.current;
    const computedStyle = window.getComputedStyle(textarea);
    const paddingTop = parseFloat(computedStyle.paddingTop);
    const paddingBottom = parseFloat(computedStyle.paddingBottom);

    // Reset height to auto to get the correct scrollHeight
    textarea.style.height = "auto";

    // Set new height
    textarea.style.height = `${textarea.scrollHeight}px`;
  }, [ref, value]);
}
