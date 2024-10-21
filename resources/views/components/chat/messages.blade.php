@foreach($messages as $message)
    <div class="mb-4 {{ $message->user_id === auth()->id() ? 'text-right' : 'text-left' }}">
        <div class="inline-block bg-gray-700 rounded-lg p-3 max-w-3/4">
            <p class="text-sm text-gray-300">{{ $message->user->name }}</p>
            <p class="text-white">{{ $message->content }}</p>
            <p class="text-xs text-gray-400">{{ $message->created_at->diffForHumans() }}</p>
        </div>
    </div>
@endforeach