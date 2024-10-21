<div id="main-content-inner">
    <div id="chat-content" class="flex flex-col h-full">
        <h2 class="text-xl font-bold mb-4">{{ $thread->title }}</h2>
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
        <div class="mt-auto">
            @include('dashboard.message-form', ['thread' => $thread])
        </div>
    </div>
</div>

<script>
    document.getElementById('message-form').addEventListener('htmx:afterRequest', function(event) {
        if (event.detail.successful) {
            event.detail.elt.reset();
        }
    });
</script>