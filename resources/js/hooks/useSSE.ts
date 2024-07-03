import { useCallback } from "react";
import { useMessageStore } from "../store";
import { Message, Codebase } from "../types";

const processStreamedContent = (
  content: string,
  isStreamingPlan: boolean,
  planBuffer: string,
  updateLastMessage: (content: string) => void,
  appendToPlan: (content: string) => void,
  updateCurrentPlan: (plan: string) => void,
): { isStreamingPlan: boolean; planBuffer: string } => {
  const planStartTag = "<plan>";
  const planEndTag = "</plan>";

  if (isStreamingPlan) {
    const endTagIndex = content.indexOf(planEndTag);
    if (endTagIndex !== -1) {
      const planContent = content.substring(0, endTagIndex);
      appendToPlan(planContent);
      updateCurrentPlan(planBuffer + planContent);
      isStreamingPlan = false;
      planBuffer = "";

      const remainingContent = content.substring(
        endTagIndex + planEndTag.length,
      );
      updateLastMessage(remainingContent);
    } else {
      planBuffer += content;
      appendToPlan(content);
    }
  } else {
    const startTagIndex = content.indexOf(planStartTag);
    if (startTagIndex !== -1) {
      const beforePlan = content.substring(0, startTagIndex);
      updateLastMessage(beforePlan);

      const afterStartTag = content.substring(
        startTagIndex + planStartTag.length,
      );
      const endTagIndex = afterStartTag.indexOf(planEndTag);
      if (endTagIndex !== -1) {
        const planContent = afterStartTag.substring(0, endTagIndex);
        updateCurrentPlan(planContent);

        const afterPlan = afterStartTag.substring(
          endTagIndex + planEndTag.length,
        );
        updateLastMessage(afterPlan);
      } else {
        isStreamingPlan = true;
        planBuffer = afterStartTag;
        updateCurrentPlan(afterStartTag);
      }
    } else {
      updateLastMessage(content);
    }
  }

  return { isStreamingPlan, planBuffer };
};

const handleSSEMessage = (
  data: any,
  processStreamedContent: (content: string) => void,
  setLastMessageComplete: () => void,
  updateLastMessage: (content: string) => void,
  addGreptileResult: (result: any) => void,
) => {
  if (data.type === "token") {
    processStreamedContent(data.content);
  } else if (data.type === "end") {
    setLastMessageComplete();
  } else if (data.type === "error") {
    console.error("Error from server:", data.content);
    updateLastMessage(
      "An error occurred while processing your request. Please try again.",
    );
    setLastMessageComplete();
  } else if (data.type === "greptile_result") {
    const greptileResult = JSON.parse(data.content);
    console.log("Parsed Greptile result:", greptileResult);
    addGreptileResult(JSON.stringify(greptileResult));
  } else if (data.type === "shell_command_result") {
    console.log("LETS EXECUTE COMMAND", data);
    const content = JSON.parse(data.content);
    console.log("Parsed shell command result:", content);
  }
};

export const useSSE = (baseUrl: string) => {
  const updateLastMessage = useMessageStore((state) => state.updateLastMessage);
  const setLastMessageComplete = useMessageStore(
    (state) => state.setLastMessageComplete,
  );
  const addMessage = useMessageStore((state) => state.addMessage);
  const updateCurrentPlan = useMessageStore((state) => state.updateCurrentPlan);
  const appendToPlan = useMessageStore((state) => state.appendToPlan);
  const addGreptileResult = useMessageStore((state) => state.addGreptileResult);

  const startSSEConnection = useCallback(
    async (messages: Message[], selectedCodebases: Codebase[]) => {
      addMessage("", false);
      let isStreamingPlan = false;
      let planBuffer = "";

      const processContent = (content: string) => {
        const result = processStreamedContent(
          content,
          isStreamingPlan,
          planBuffer,
          updateLastMessage,
          appendToPlan,
          updateCurrentPlan,
        );
        isStreamingPlan = result.isStreamingPlan;
        planBuffer = result.planBuffer;
      };

      try {
        const response = await fetch(baseUrl, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Accept: "text/event-stream",
          },
          body: JSON.stringify({ messages, codebases: selectedCodebases }),
        });

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const reader = response.body!.getReader();
        const decoder = new TextDecoder();

        while (true) {
          const { value, done } = await reader.read();
          if (done) break;

          const chunk = decoder.decode(value);
          const lines = chunk.split("\n");

          for (const line of lines) {
            if (line.startsWith("data: ")) {
              const data = JSON.parse(line.slice(6));
              handleSSEMessage(
                data,
                processContent,
                setLastMessageComplete,
                updateLastMessage,
                addGreptileResult,
              );
            }
          }
        }
      } catch (error) {
        console.error("SSE error:", error);
        updateLastMessage(
          "An error occurred while connecting to the server. Please try again.",
        );
        setLastMessageComplete();
      }
    },
    [
      baseUrl,
      updateLastMessage,
      setLastMessageComplete,
      addMessage,
      updateCurrentPlan,
      appendToPlan,
      addGreptileResult,
    ],
  );

  return { startSSEConnection };
};
