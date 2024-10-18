# Chat System Implementation Log

## Step 1: Update MessageController

Date: [Current Date]

Updated the `MessageController` to handle HTMX requests and prepare for SSE setup.

### Changes made:

1. Modified the `sendMessage` method in `app/Http/Controllers/MessageController.php`:

```php
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
```

2. Added a new method `streamResponse` to handle SSE connections:

```php
public function streamResponse(Thread $thread)
{
    return response()->stream(function() use ($thread) {
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
```

### Next steps:
1. Update the routes to include the new SSE endpoint.
2. Modify the chat view to incorporate HTMX and SSE attributes.
3. Test the changes to ensure proper functionality.

## Step 2: Update Routes

Date: [Current Date]

Added a new route for SSE connections in `routes/web.php`:

```php
use App\Http\Controllers\MessageController;

// Existing routes...

Route::get('/chat/{thread}/stream', [MessageController::class, 'streamResponse'])->name('chat.stream');
```

### Next steps:
1. Modify the chat view to incorporate HTMX and SSE attributes.
2. Test the changes to ensure proper functionality.

## Step 3: Update Chat View

Date: [Current Date]

Modified the chat view to incorporate HTMX and SSE attributes. Updated `resources/views/chat/show.blade.php`:

```html
@extends('layouts.app')

@section('content')
<div class="container">
    <h1>Chat Thread: {{ $thread->title }}</h1>
    
    <div id="chat-messages" hx-ext="sse" sse-connect="{{ route('chat.stream', ['thread' => $thread->id]) }}">
        <div sse-swap="message">
            <!-- Existing messages will be loaded here -->
            @foreach ($thread->messages as $message)
                @include('partials.message', ['message' => $message])
            @endforeach
        </div>
    </div>

    <form hx-post="{{ route('messages.store') }}" hx-target="#chat-messages" hx-swap="beforeend">
        @csrf
        <input type="hidden" name="thread_id" value="{{ $thread->id }}">
        <div class="form-group">
            <textarea name="content" class="form-control" rows="3" required></textarea>
        </div>
        <button type="submit" class="btn btn-primary">Send</button>
    </form>
</div>
@endsection

@push('scripts')
<script src="https://unpkg.com/htmx-ext-sse@2.2.2/sse.js"></script>
@endpush
```

### Changes made:
1. Added HTMX and SSE attributes to the chat messages container.
2. Updated the form to use HTMX for submission.
3. Included the HTMX SSE extension script.

### Next steps:
1. Test the changes to ensure proper functionality.
2. Implement error handling and reconnection logic for SSE.
3. Optimize the streaming response for better performance.
