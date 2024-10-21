<div class="bg-zinc-800 text-white p-4">
    <h2 class="text-xl font-bold mb-4">Recent Threads</h2>
    <p>Recent Threads Count: {{ $recentThreads->count() }}</p>
    <ul>
        @forelse ($recentThreads as $thread)
        <li class="mb-2">
            <a href="{{ route('threads.show', $thread) }}" 
               class="hover:text-zinc-300"
               hx-get="{{ route('threads.show', $thread) }}"
               hx-target="#main-content"
               hx-push-url="true">
                {{ Str::limit($thread->title, 30) }}
            </a>
        </li>
        @empty
        <li>No recent threads</li>
        @endforelse
    </ul>
</div>