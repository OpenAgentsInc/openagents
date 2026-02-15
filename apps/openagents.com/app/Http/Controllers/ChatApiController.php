<?php

namespace App\Http\Controllers;

use App\AI\Agents\AutopilotAgent;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Illuminate\Validation\ValidationException;

class ChatApiController extends Controller
{
    public function stream(Request $request)
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $conversationId = $request->query('conversationId');

        if (! is_string($conversationId) || $conversationId === '') {
            throw ValidationException::withMessages([
                'conversationId' => ['conversationId query param is required'],
            ]);
        }

        $conversationExists = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->exists();

        if (! $conversationExists) {
            abort(404);
        }

        $payload = $request->validate([
            'messages' => ['required', 'array', 'min:1'],
            'messages.*.role' => ['required', 'string'],
            'messages.*.content' => ['required', 'string'],
        ]);

        // The Vercel AI SDK sends the full message list; we take the last user message as the next prompt.
        $prompt = null;
        foreach (array_reverse($payload['messages']) as $m) {
            if (($m['role'] ?? null) === 'user') {
                $prompt = (string) ($m['content'] ?? '');
                break;
            }
        }

        if ($prompt === null || $prompt === '') {
            throw ValidationException::withMessages([
                'messages' => ['A user message is required'],
            ]);
        }

        $agent = AutopilotAgent::make()->continue($conversationId, $user);

        return $agent->stream($prompt)->usingVercelDataProtocol();
    }
}
