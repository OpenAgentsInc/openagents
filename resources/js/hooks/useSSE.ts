import { useCallback } from "react";
import { useMessageStore } from "../store";

interface Message {
  role: "user" | "assistant";
  content: string;
}

interface Codebase {
  id: string;
  name: string;
  branch: string;
}

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
      addMessage("", false); // Add an empty message for the AI response
      let isStreamingPlan = false;
      let planBuffer = "";

      const processStreamedContent = (content: string) => {
        console.log("Processing streamed content:", content);
        console.log("Current isStreamingPlan state:", isStreamingPlan);

        const planStartTag = "<plan>";
        const planEndTag = "</plan>";

        if (isStreamingPlan) {
          const endTagIndex = content.indexOf(planEndTag);
          if (endTagIndex !== -1) {
            // End of plan found
            const planContent = content.substring(0, endTagIndex);
            appendToPlan(planContent);
            updateCurrentPlan(planBuffer + planContent);
            isStreamingPlan = false;
            planBuffer = "";

            // Process the rest of the content after </plan>
            const remainingContent = content.substring(
              endTagIndex + planEndTag.length,
            );
            updateLastMessage(remainingContent);
            console.log("Plan ended. Remaining content:", remainingContent);
          } else {
            // Still in plan, append to buffer
            planBuffer += content;
            appendToPlan(content);
          }
        } else {
          const startTagIndex = content.indexOf(planStartTag);
          if (startTagIndex !== -1) {
            // Start of plan found
            const beforePlan = content.substring(0, startTagIndex);
            updateLastMessage(beforePlan);

            // Process the plan content
            const afterStartTag = content.substring(
              startTagIndex + planStartTag.length,
            );
            const endTagIndex = afterStartTag.indexOf(planEndTag);
            if (endTagIndex !== -1) {
              // Complete plan in this chunk
              const planContent = afterStartTag.substring(0, endTagIndex);
              updateCurrentPlan(planContent);

              // Process content after the plan
              const afterPlan = afterStartTag.substring(
                endTagIndex + planEndTag.length,
              );
              updateLastMessage(afterPlan);
              console.log(
                "Complete plan found in chunk. After plan:",
                afterPlan,
              );
            } else {
              // Start of plan, but not complete
              isStreamingPlan = true;
              planBuffer = afterStartTag;
              updateCurrentPlan(afterStartTag);
              console.log("Start of plan found, streaming begins");
            }
          } else {
            // Regular content
            updateLastMessage(content);
            console.log("Regular content:", content);
          }
        }
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
              if (data.type === "token") {
                processStreamedContent(data.content);
              } else if (data.type === "end") {
                setLastMessageComplete();
                return;
              } else if (data.type === "error") {
                console.error("Error from server:", data.content);
                updateLastMessage(
                  "An error occurred while processing your request. Please try again.",
                );
                setLastMessageComplete();
                return;
              } else if (data.type === "greptile_result") {
                console.log("Full Greptile result:", data);
                const greptileResult = JSON.parse(data.content);
                if (
                  Array.isArray(greptileResult.content) &&
                  greptileResult.content.length > 0
                ) {
                  const summary = greptileResult.content[0].summary;
                  addGreptileResult(summary);
                } else {
                  console.error(
                    "Unexpected Greptile result format:",
                    greptileResult,
                  );
                  addGreptileResult(
                    "Unable to extract summary from Greptile result.",
                  );
                }
              }
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
