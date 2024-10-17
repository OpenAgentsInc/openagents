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

        return $this->streamResponse($message);
    }

    private function streamResponse(Message $message)
    {
        return response()->stream(function() use ($message) {
            $userMessageHtml = view('partials.message', ['message' => $message])->render();
            echo "data: " . json_encode(['html' => $userMessageHtml]) . "\n\n";
            ob_flush();
            flush();

            $demoResponses = [
                'This is a demo SSE message',
                'Another demo SSE message',
                'Final demo SSE message',
            ];

            foreach ($demoResponses as $index => $content) {
                usleep(500000); // 0.5 second delay
                $demoMessage = new Message();
                $demoMessage->content = $content;
                $demoMessage->is_system_message = true;
                $demoMessage->created_at = now()->addSeconds(($index + 1) * 0.5);
                
                $demoMessageHtml = view('partials.message', ['message' => $demoMessage])->render();
                echo "data: " . json_encode(['html' => $demoMessageHtml]) . "\n\n";
                ob_flush();
                flush();
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
}