<?php

namespace App\Http\Controllers;

use App\Models\Thread;
use Illuminate\Http\RedirectResponse;
use Illuminate\Support\Facades\Auth;
use Inertia\Inertia;
use Inertia\Response;

class ChatController
{
    public function index(): RedirectResponse
    {
        $query = Thread::query();
        
        if (Auth::user()->current_team_id) {
            $query->where('team_id', Auth::user()->current_team_id);
        } else {
            $query->where('user_id', Auth::id())
                  ->whereNull('team_id');
        }
        
        $latestThread = $query->orderBy('created_at', 'desc')->first();

        if (!$latestThread) {
            return redirect()->route('chat.create');
        }

        return redirect()->route('chat.id', $latestThread->id);
    }

    public function create(): RedirectResponse
    {
        $user = Auth::user();
        
        $thread = Thread::create([
            'user_id' => $user->id,
            'team_id' => $user->current_team_id,
            'title' => 'New Chat',
        ]);

        return redirect()->route('chat.id', $thread->id);
    }

    public function show($id): Response
    {
        $thread = Thread::with('messages.toolInvocations')->findOrFail($id);
        $user = Auth::user();

        // Check if user has access to this thread
        if ($thread->team_id) {
            // Team thread - verify user belongs to the team
            if (!$user->teams->contains('id', $thread->team_id)) {
                abort(403, 'You do not have access to this team thread.');
            }
        } else {
            // Personal thread - verify ownership
            if ($thread->user_id !== $user->id) {
                abort(403, 'You do not have access to this thread.');
            }
        }

        return Inertia::render('Chat', [
            'messages' => $thread->messages->map(function ($message) {
                return array_merge($message->toArray(), [
                    'toolInvocations' => $message->toolInvocations
                ]);
            }),
            'currentChatId' => $thread->id,
        ]);
    }

    public function destroy($id): RedirectResponse
    {
        $thread = Thread::findOrFail($id);
        $user = Auth::user();

        // Check if user has permission to delete this thread
        if ($thread->team_id) {
            // Team thread - verify user belongs to the team
            if (!$user->teams->contains('id', $thread->team_id)) {
                abort(403, 'You do not have permission to delete this team thread.');
            }
        } else {
            // Personal thread - verify ownership
            if ($thread->user_id !== $user->id) {
                abort(403, 'You do not have permission to delete this thread.');
            }
        }

        $thread->delete();

        return redirect('/chat');
    }
}