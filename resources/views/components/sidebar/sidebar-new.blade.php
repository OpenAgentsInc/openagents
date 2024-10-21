<div class="bg-gray-800 text-white p-4 h-screen">
    <h2 class="text-xl font-bold mb-4">Recent Threads</h2>
    @if(isset($recentThreads))
        <p>Recent Threads is set</p>
        <p>Count: {{ $recentThreads->count() }}</p>
    @else
        <p>Recent Threads is not set</p>
    @endif
    <ul>
        @forelse ($recentThreads ?? [] as $thread)
            <li class="mb-2">
                <a href="{{ route('threads.show', $thread) }}" class="hover:text-gray-300">
                    {{ Str::limit($thread->title, 30) }}
                </a>
            </li>
        @empty
            <li>No recent threads</li>
        @endforelse
    </ul>
</div>