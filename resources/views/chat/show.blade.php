<div id="chat-content" class="flex flex-col h-full">
    <div class="flex-grow overflow-y-auto" id="message-list">
        @foreach($messages as $message)
            <div class="mb-4 @if($message->user_id === auth()->id()) text-right @endif">
                <div class="inline-block bg-gray-200 rounded-lg px-4 py-2 max-w-3/4">
                    <p class="text-sm">{{ $message->content }}</p>
                </div>
                <p class="text-xs text-gray-500 mt-1">{{ $message->created_at->diffForHumans() }}</p>
            </div>
        @endforeach
    </div>
    <div class="mt-4">
        <form id="message-form" hx-post="{{ route('threads.addMessage', $thread) }}" hx-swap="afterbegin" hx-target="#message-list">
            <div class="flex">
                <input type="text" name="content" class="flex-grow rounded-l-lg border-t border-b border-l text-gray-800 border-gray-200 bg-white px-4 py-2" placeholder="Type your message...">
                <button type="submit" class="rounded-r-lg bg-blue-500 text-white px-4 py-2 font-semibold">Send</button>
            </div>
        </form>
    </div>
</div>

<script>
    document.getElementById('message-form').addEventListener('htmx:afterRequest', function(event) {
        if (event.detail.successful) {
            event.detail.elt.reset();
        }
    });
</script>