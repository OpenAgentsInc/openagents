import { createFileRoute } from '@tanstack/react-router';
import { openai } from '@ai-sdk/openai';
import { streamText, convertToModelMessages, type UIMessage } from 'ai';

// Set OPENAI_API_KEY (e.g. wrangler secret put OPENAI_API_KEY) for the API to work.
export const Route = createFileRoute('/api/chat')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const { messages } = (await request.json()) as { messages: UIMessage[] };

        const result = streamText({
          model: openai.responses('gpt-4o-mini'),
          messages: await convertToModelMessages(messages),
        });

        return result.toUIMessageStreamResponse();
      },
    },
  },
});
