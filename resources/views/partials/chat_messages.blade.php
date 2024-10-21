@if(count($messages) > 0)
    <div class="space-y-4">
        @foreach($messages as $message)
            <div class="mb-4 {{ $message->user_id == auth()->id() ? 'text-right' : 'text-left' }}">
                <div class="inline-block bg-zinc-800 text-white rounded px-4 py-2">
                    <p class="text-sm">{{ $message->content }}</p>
                    <p class="text-xs text-zinc-400">{{ $message->created_at->format('M d, Y H:i') }}</p>
                </div>
            </div>
        @endforeach
    </div>
@else
    <x-empty-message-list />
@endif