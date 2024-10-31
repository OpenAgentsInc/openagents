<?php

namespace App\Http\Controllers;

use App\Models\Project;
use App\Models\Thread;
use Illuminate\Http\RedirectResponse;
use Illuminate\Support\Facades\Auth;
use Inertia\Inertia;
use Inertia\Response;

class ChatController
{
    public function index(): RedirectResponse
    {
        $user = Auth::user();
        $query = Thread::query();
        
        if ($user->current_team_id) {
            // In team context, get threads from team's projects
            $query->whereHas('project', function ($q) use ($user) {
                $q->where('team_id', $user->current_team_id);
            });
        } else {
            // In personal context, get threads without projects or owned by user
            $query->where(function ($q) use ($user) {
                $q->whereNull('project_id')
                  ->where('user_id', $user->id);
            });
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
        $data = [
            'user_id' => $user->id,
            'title' => 'New Chat',
        ];

        // If in team context, create thread under default project
        if ($user->current_team_id) {
            $project = Project::firstOrCreate(
                ['team_id' => $user->current_team_id, 'is_default' => true],
                ['name' => 'Default Project']
            );
            $data['project_id'] = $project->id;
        }

        $thread = Thread::create($data);

        return redirect()->route('chat.id', $thread->id);
    }

    public function show($id): Response
    {
        $thread = Thread::with(['messages.toolInvocations', 'project.team'])->findOrFail($id);
        $user = Auth::user();

        // Check if user has access to this thread
        if ($thread->project && $thread->project->team_id) {
            // Team thread - verify user belongs to the team
            if (!$user->teams->contains('id', $thread->project->team_id)) {
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
        $thread = Thread::with('project.team')->findOrFail($id);
        $user = Auth::user();

        // Check if user has permission to delete this thread
        if ($thread->project && $thread->project->team_id) {
            // Team thread - verify user belongs to the team
            if (!$user->teams->contains('id', $thread->project->team_id)) {
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