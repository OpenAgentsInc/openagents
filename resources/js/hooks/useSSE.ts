import { useCallback } from "react";
import { useMessageStore } from "../store";

interface Message {
  role: "user" | "assistant";
  content: string;
}

export const useSSE = (baseUrl: string) => {
  const updateLastMessage = useMessageStore((state) => state.updateLastMessage);
  const setLastMessageComplete = useMessageStore(
    (state) => state.setLastMessageComplete
  );
  const addMessage = useMessageStore((state) => state.addMessage);
  const updateCurrentPlan = useMessageStore((state) => state.updateCurrentPlan);
  const appendToPlan = useMessageStore((state) => state.appendToPlan);

  const startSSEConnection = useCallback(
    async (messages: Message[]) => {
      addMessage("", false); // Add an empty message for the AI response
      let isStreamingPlan = false;

      const processStreamedContent = (content: string) => {
        console.log("Processing streamed content:", content);
        console.log("Current isStreamingPlan state:", isStreamingPlan);

        const planStartTag = "<plan>";
        const planEndTag = "</plan>";
        const planStartIndex = content.indexOf(planStartTag);
        const planEndIndex = content.indexOf(planEndTag);

        console.log("Plan start index:", planStartIndex);
        console.log("Plan end index:", planEndIndex);

        if (planStartIndex !== -1) {
          if (planEndIndex !== -1) {
            // Complete plan
            console.log("Found complete plan");
            const beforePlan = content.substring(0, planStartIndex);
            const plan = content.substring(
              planStartIndex + planStartTag.length,
              planEndIndex
            );
            const afterPlan = content.substring(
              planEndIndex + planEndTag.length
            );

            console.log("Before plan:", beforePlan);
            console.log("Plan content:", plan);
            console.log("After plan:", afterPlan);

            updateLastMessage(beforePlan + afterPlan);
            updateCurrentPlan(plan);
            isStreamingPlan = false;
          } else {
            // Start of plan
            console.log("Found start of plan");
            const beforePlan = content.substring(0, planStartIndex);
            const incompletePlan = content.substring(
              planStartIndex + planStartTag.length
            );

            console.log("Before plan:", beforePlan);
            console.log("Incomplete plan:", incompletePlan);

            updateLastMessage(beforePlan);
            updateCurrentPlan(incompletePlan);
            isStreamingPlan = true;
          }
        } else if (isStreamingPlan) {
          // Continuing plan
          console.log("Continuing plan:", content);
          appendToPlan(content);
        } else {
          // Regular content
          console.log("Regular content:", content);
          updateLastMessage(content);
        }
      };

      try {
        const response = await fetch(baseUrl, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Accept: "text/event-stream",
          },
          body: JSON.stringify({ messages }),
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
                  "An error occurred while processing your request. Please try again."
                );
                setLastMessageComplete();
                return;
              }
            }
          }
        }
      } catch (error) {
        console.error("SSE error:", error);
        updateLastMessage(
          "An error occurred while connecting to the server. Please try again."
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
    ]
  );

  return { startSSEConnection };
};
