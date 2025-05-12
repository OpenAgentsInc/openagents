import { OpenAIStream, StreamingTextResponse } from 'ai';
import OpenAI from 'openai';
import { getIssueById } from '@/lib/db/issue-helpers.server';

// Create an OpenAI API client (that's edge friendly!)
const openai = new OpenAI({
  apiKey: process.env.OPENAI_API_KEY || '',
});

export async function POST(req: Request, { params }: { params: { id: string } }) {
  const { messages } = await req.json();
  const issueId = params.id;

  // Fetch the issue details
  const issue = await getIssueById(issueId);
  if (!issue) {
    return new Response('Issue not found', { status: 404 });
  }

  // Create system message with issue context
  const systemMessage = {
    role: 'system',
    content: `You are a helpful AI assistant discussing issue ${issue.identifier}: "${issue.title}".
    The issue was created on ${issue.createdAt} and is currently in the "${issue.status.name}" state.
    ${issue.description ? `The issue description is: "${issue.description}"` : 'No description is provided.'}`
  };

  // Add system message to the beginning of the messages array
  const augmentedMessages = [systemMessage, ...messages];

  // Ask OpenAI for a streaming chat completion
  const response = await openai.chat.completions.create({
    model: 'gpt-4',
    stream: true,
    messages: augmentedMessages,
  });

  // Convert the response into a friendly text-stream
  const stream = OpenAIStream(response);

  // Return a StreamingTextResponse, which can be consumed by the client
  return new StreamingTextResponse(stream);
}
