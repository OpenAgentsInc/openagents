<div class="mx-auto flex h-full w-full flex-col text-base justify-between md:max-w-3xl">
    <div id="message-list" class="flex-grow overflow-y-auto space-y-4">
        <h2 class="text-xl font-bold mb-4">{{ $thread->title }}</h2>
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
        @include('dashboard.message-form', ['thread' => $thread])
        @include('dashboard.terms-privacy')
    </div>
</div>

<script>
    document.addEventListener('DOMContentLoaded', function() {
        const messageForm = document.getElementById('message-form');
        if (messageForm) {
            messageForm.addEventListener('htmx:afterRequest', function(event) {
                if (event.detail.successful) {
                    event.detail.elt.reset();
                }
            });
        }
    });
</script>