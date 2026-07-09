import { defineChannel, POST, type Session } from "eve/channels";
import { z } from "zod";

const transcriptSchema = z.object({
  modality: z.enum(["text", "voice"]),
  role: z.enum(["user", "assistant"]),
  sourceEvent: z.string().min(1),
  text: z.string().min(1),
  threadId: z.string().min(1),
  prospectRef: z.string().min(1),
  clientThreadId: z.string().min(1).optional(),
});

function transcriptMessage(input: z.infer<typeof transcriptSchema>) {
  return [
    "[Realtime transcript bridge]",
    `Prospect ref: ${input.prospectRef}`,
    `Thread: ${input.threadId}`,
    input.clientThreadId ? `Client thread: ${input.clientThreadId}` : null,
    `Modality: ${input.modality}`,
    `Speaker: ${input.role}`,
    `Source event: ${input.sourceEvent}`,
    "",
    input.text,
  ]
    .filter(Boolean)
    .join("\n");
}

async function waitForBoundary(session: Session) {
  const stream = await session.getEventStream();
  const reader = stream.getReader();
  const timeoutMs = 30_000;

  try {
    for (;;) {
      const result = await Promise.race([
        reader.read(),
        new Promise<"timeout">((resolve) =>
          setTimeout(() => resolve("timeout"), timeoutMs),
        ),
      ]);

      if (result === "timeout") return "timeout";
      if (result.done) return "stream-ended";

      const type = result.value.type;
      if (
        type === "session.waiting" ||
        type === "session.completed" ||
        type === "session.failed"
      ) {
        return type;
      }
    }
  } finally {
    await reader.cancel().catch(() => undefined);
  }
}

type RealtimeTranscriptState = {
  prospectRef: string;
  threadId: string;
};

export default defineChannel<RealtimeTranscriptState>({
  routes: [
    POST("/eve/realtime/transcript", async (request, { send }) => {
      const parsed = transcriptSchema.safeParse(await request.json());

      if (!parsed.success) {
        return Response.json(
          { error: "Invalid Sarah transcript turn." },
          { status: 400 },
        );
      }

      const session = await send(transcriptMessage(parsed.data), {
        auth: null,
        continuationToken: parsed.data.threadId,
        state: {
          prospectRef: parsed.data.prospectRef,
          threadId: parsed.data.threadId,
        },
        title: `Sarah realtime transcript ${parsed.data.threadId}`,
      });
      const boundary = await waitForBoundary(session);

      return Response.json({
        ok: true,
        boundary,
        sessionId: session.id,
        threadId: parsed.data.threadId,
      });
    }),
  ],
});
