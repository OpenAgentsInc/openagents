<?php

namespace App\Http\Controllers;

use App\Models\Message;
use App\Models\Thread;
use App\Models\Project;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Response;
use Symfony\Component\HttpFoundation\StreamedResponse;

class MessageController extends Controller
{
    public function store(Request $request)
    {
        $validatedData = $request->validate([
            'thread_id' => 'required|exists:threads,id',
            'content' => 'required|string',
        ]);

        $message = Message::create([
            'thread_id' => $validatedData['thread_id'],
            'user_id' => auth()->id(),
            'content' => $validatedData['content'],
        ]);

        return response()->json($message, 201);
    }

    public function storeInThread(Request $request, Thread $thread)
    {
        $validatedData = $request->validate([
            'content' => 'required|string',
        ]);

        $message = $thread->messages()->create([
            'user_id' => $request->input('user_id', auth()->id()),
            'content' => $validatedData['content'],
        ]);

        return response()->json($message, 201);
    }

    public function sendMessage(Request $request)
    {
        $request->validate([
            'message' => 'required|string|max:1000',
            'project_id' => 'nullable|exists:projects,id',
        ]);

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
            
            if (auth()->check()) {
                $thread->user_id = auth()->id();
            }
            
            $thread->save();
        }

        $message = new Message();
        $message->thread_id = $thread->id;
        $message->content = $request->message;
        $message->is_system_message = false;
        
        if (auth()->check()) {
            $message->user_id = auth()->id();
        }
        
        $message->save();

        return response()->json(['message' => 'Message sent successfully!', 'html' => view('partials.message', ['message' => $message])->render()]);
    }

    public function sseDemo()
    {
        $response = new StreamedResponse(function() {
            echo "data: " . json_encode(['html' => '<div class="message">This is a demo SSE message</div>']) . "\n\n";
            ob_flush();
            flush();
            sleep(2);

            echo "data: " . json_encode(['html' => '<div class="message">Another demo SSE message</div>']) . "\n\n";
            ob_flush();
            flush();
            sleep(2);

            echo "data: " . json_encode(['html' => '<div class="message">Final demo SSE message</div>']) . "\n\n";
            ob_flush();
            flush();
        });

        $response->headers->set('Content-Type', 'text/event-stream');
        $response->headers->set('Cache-Control', 'no-cache');
        $response->headers->set('Connection', 'keep-alive');

        return $response;
    }
}