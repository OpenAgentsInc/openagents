<?php

namespace App\Http\Controllers;

use App\AI\RunOrchestrator;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class ChatApiController extends Controller
{
    public function stream(Request $request)
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $conversationId = $request->route('conversationId');

        if (! is_string($conversationId) || trim($conversationId) === '') {
            $conversationId = $request->query('conversationId');
        }

        if (! is_string($conversationId) || trim($conversationId) === '') {
            return $this->unprocessable('conversationId is required (route param or query param)');
        }

        $conversationId = trim($conversationId);

        $conversationExists = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->exists();

        if (! $conversationExists) {
            abort(404);
        }

        $rawMessages = $request->input('messages');

        if (! is_array($rawMessages) || $rawMessages === []) {
            return $this->unprocessable('messages must be a non-empty array');
        }

        $messages = $this->normalizeMessages($rawMessages);

        if ($messages === []) {
            return $this->unprocessable('messages must include at least one valid message');
        }

        // The AI SDK sends the full message list; use the most recent user message as the next prompt.
        $prompt = null;
        foreach (array_reverse($messages) as $m) {
            if (($m['role'] ?? null) === 'user' && trim((string) ($m['content'] ?? '')) !== '') {
                $prompt = trim((string) $m['content']);
                break;
            }
        }

        if ($prompt === null || $prompt === '') {
            return $this->unprocessable('A non-empty user message is required');
        }

        // PostHog: Track chat message sent
        $posthog = resolve(PostHogService::class);
        $posthog->capture($user->email, 'chat message sent', [
            'conversation_id' => $conversationId,
            'message_length' => strlen($prompt),
        ]);

        $orchestrator = resolve(RunOrchestrator::class);

        return $orchestrator->streamAutopilotRun($user, $conversationId, $prompt);
    }

    /**
     * @param  array<int, mixed>  $rawMessages
     * @return array<int, array{role: string, content: string}>
     */
    private function normalizeMessages(array $rawMessages): array
    {
        $normalized = [];

        foreach ($rawMessages as $rawMessage) {
            if (! is_array($rawMessage)) {
                continue;
            }

            $role = $rawMessage['role'] ?? null;
            if (! is_string($role) || trim($role) === '') {
                continue;
            }

            $content = '';

            if (isset($rawMessage['content']) && is_string($rawMessage['content'])) {
                $content = $rawMessage['content'];
            } elseif (isset($rawMessage['parts']) && is_array($rawMessage['parts'])) {
                $content = $this->contentFromParts($rawMessage['parts']);
            }

            $normalized[] = [
                'role' => trim($role),
                'content' => trim($content),
            ];
        }

        return $normalized;
    }

    /**
     * @param  array<int, mixed>  $parts
     */
    private function contentFromParts(array $parts): string
    {
        $chunks = [];

        foreach ($parts as $part) {
            if (! is_array($part)) {
                continue;
            }

            $text = $part['text'] ?? null;
            if (is_string($text) && $text !== '') {
                $chunks[] = $text;
            }
        }

        return implode('', $chunks);
    }

    private function unprocessable(string $message): JsonResponse
    {
        return response()->json([
            'message' => $message,
            'errors' => [
                'messages' => [$message],
            ],
        ], 422);
    }
}
