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

        return redirect()->route('chat.show', ['thread' => $thread->id])->with('success', 'Message sent successfully!');
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
            $systemMessage->content = ''; // We'll accumulate the content
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
                $systemMessage->content .= $word . ' ';
                echo "data: " . json_encode(['type' => 'word', 'content' => $word . ' ']) . "\n\n";
                ob_flush();
                flush();
            }

            // Save the complete system message
            $systemMessage->save();

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