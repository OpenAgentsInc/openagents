<div class="p-4">
    @foreach($messages as $message)
        <div class="mb-4 {{ $message->user_id == auth()->id() ? 'text-right' : 'text-left' }}">
            <div class="inline-block bg-gray-200 rounded px-4 py-2">
                <p class="text-sm">{{ $message->content }}</p>
                <p class="text-xs text-gray-500">{{ $message->created_at->format('M d, Y H:i') }}</p>
            </div>
        </div>
    @endforeach
</div>