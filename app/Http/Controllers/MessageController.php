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

        $userMessage = new Message();
        $userMessage->thread_id = $thread->id;
        $userMessage->content = $request->message;
        $userMessage->is_system_message = false;
        
        if (auth()->check()) {
            $userMessage->user_id = auth()->id();
        }
        
        $userMessage->save();

        return $this->streamResponse($userMessage);
    }

    private function streamResponse(Message $userMessage)
    {
        return response()->stream(function() use ($userMessage) {
            // Send user message
            $userMessageHtml = view('partials.message', ['message' => $userMessage])->render();
            echo "data: " . json_encode(['type' => 'user', 'html' => $userMessageHtml]) . "\n\n";
            ob_flush();
            flush();

            // Create system message
            $systemMessage = new Message();
            $systemMessage->thread_id = $userMessage->thread_id;
            $systemMessage->is_system_message = true;
            $systemMessage->content = ''; // We'll stream the content word by word
            $systemMessage->save();

            $systemMessageHtml = view('partials.message', ['message' => $systemMessage])->render();
            echo "data: " . json_encode(['type' => 'system', 'html' => $systemMessageHtml]) . "\n\n";
            ob_flush();
            flush();

            // Demo response to stream word by word
            $demoResponse = "This is a demo response that will be streamed word by word to simulate an AI generating a response in real-time.";
            $words = explode(' ', $demoResponse);

            foreach ($words as $word) {
                usleep(200000); // 0.2 second delay between words
                echo "data: " . json_encode(['type' => 'word', 'content' => $word . ' ']) . "\n\n";
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