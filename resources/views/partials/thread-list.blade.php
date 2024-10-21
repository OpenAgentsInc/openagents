@if(isset($message))
<p class="text-zinc-500 dark:text-zinc-400">{{ $message }}</p>
@elseif($threads->isEmpty())
<p class="text-zinc-500 dark:text-zinc-400">No threads found.</p>
@else
<ul class="space-y-2">
    @foreach($threads as $thread)
    <li>
        <a href="#"
            class="block p-2 rounded-md hover:bg-zinc-100 dark:hover:bg-zinc-700"
            hx-get="{{ route('threads.messages', $thread) }}"
            hx-target="#main-content-inner"
            hx-swap="innerHTML">
            <div class="flex items-center justify-between">
                <span class="text-sm font-medium text-zinc-900 dark:text-white">{{ $thread->title }}</span>
                <span class="text-xs text-zinc-500 dark:text-zinc-400">{{ $thread->updated_at->diffForHumans() }}</span>
            </div>
            <p class="mt-1 text-xs text-zinc-500 dark:text-zinc-400 truncate">{{ $thread->messages->last()->content ?? 'No messages' }}</p>
        </a>
    </li>
    @endforeach
</ul>
{{ $threads->links() }}
@endif
