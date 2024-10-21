@if(isset($message))
    <p class="text-gray-500 dark:text-gray-400">{{ $message }}</p>
@elseif($threads->isEmpty())
    <p class="text-gray-500 dark:text-gray-400">No threads found.</p>
@else
    <ul class="space-y-2">
        @foreach($threads as $thread)
            <li>
                <a href="{{ route('chat.show', $thread) }}" class="block p-2 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700">
                    <div class="flex items-center justify-between">
                        <span class="text-sm font-medium text-gray-900 dark:text-white">{{ $thread->title }}</span>
                        <span class="text-xs text-gray-500 dark:text-gray-400">{{ $thread->updated_at->diffForHumans() }}</span>
                    </div>
                    <p class="mt-1 text-xs text-gray-500 dark:text-gray-400 truncate">{{ $thread->messages->last()->content ?? 'No messages' }}</p>
                </a>
            </li>
        @endforeach
    </ul>
    {{ $threads->links() }}
@endif