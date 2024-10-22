<?php

namespace App\Http\Controllers;

use App\Models\Message;
use App\Models\Thread;
use App\Models\Project;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Response;
use Symfony\Component\HttpFoundation\StreamedResponse;
use Illuminate\Validation\ValidationException;

class MessageController extends Controller
{
    public function sendMessage(Request $request)
    {
        try {
            $validated = $request->validate([
                'message' => 'required|string|max:1000',
                'project_id' => 'nullable|exists:projects,id',
                'thread_id' => 'nullable|exists:threads,id',
            ]);
        } catch (ValidationException $e) {
            return response()->json(['errors' => $e->errors()], 422);
        }

        $thread = null;
        if ($request->has('thread_id')) {
            $thread = Thread::findOrFail($request->thread_id);
        } else {
            $thread = new Thread();
            $thread->title = substr($request->message, 0, 50) . '...';

            if ($request->has('project_id')) {
                $project = Project::findOrFail($request->project_id);
                $thread->project_id = $project->id;
            }

            $thread->user_id = auth()->id();
            $thread->save();
        }

        $userMessage = new Message();
        $userMessage->thread_id = $thread->id;
        $userMessage->content = $request->message;
        $userMessage->is_system_message = false;
        $userMessage->user_id = auth()->id();
        $userMessage->save();

        // Check if the request is an HTMX request
        if ($request->header('HX-Request')) {
            // Return an HTMX response that triggers a redirect
            return response()->json([
                'HX-Redirect' => route('chat.show', ['thread' => $thread->id])
            ]);
        }

        // For non-HTMX requests, return a regular redirect
        return redirect()->route('chat.show', ['thread' => $thread->id])->with('success', 'Message sent successfully!');
    }

    public function streamResponse(Thread $thread)
    {
        return response()->stream(function () use ($thread) {
            $messages = $thread->messages()->orderBy('created_at', 'asc')->get();

            foreach ($messages as $message) {
                $messageHtml = view('partials.message', ['message' => $message])->render();
                echo "data: " . json_encode(['html' => $messageHtml]) . "\n\n";
                ob_flush();
                flush();
                usleep(100000); // Simulate delay between messages
            }

            echo "data: [DONE]\n\n";
            ob_flush();
            flush();
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
            'Connection' => 'keep-alive',
        ]);
    }

    public function store(Request $request)
    {
        try {
            $validated = $request->validate([
                'thread_id' => 'required|exists:threads,id',
                'content' => 'required|string|max:1000',
            ]);
        } catch (ValidationException $e) {
            return response()->json(['errors' => $e->errors()], 422);
        }

        $message = new Message();
        $message->thread_id = $request->thread_id;
        $message->content = $request->content;
        $message->is_system_message = false;
        $message->user_id = auth()->id();
        $message->save();

        return response()->json($message, 201);
    }

    public function storeInThread(Request $request, Thread $thread)
    {
        try {
            $validated = $request->validate([
                'content' => 'required|string|max:1000',
                'user_id' => 'nullable|exists:users,id',
            ]);
        } catch (ValidationException $e) {
            return response()->json(['errors' => $e->errors()], 422);
        }

        $message = new Message();
        $message->thread_id = $thread->id;
        $message->content = $request->content;
        $message->is_system_message = $request->user_id === null;
        $message->user_id = $request->user_id ?? ($message->is_system_message ? null : auth()->id());
        $message->save();

        return response()->json($message, 201);
    }
}
