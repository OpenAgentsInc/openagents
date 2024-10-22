<div class="text-white">
    <x-sidebar.section-platform />
    <x-sidebar.section-projects />
    <!--
    <h2 class="text-xl font-bold mb-4">Recent Threads</h2>
    <p>Recent Threads Count: {{ $recentThreads->count() }}</p>
    <ul>
        @forelse ($recentThreads as $thread)
        <li class="mb-2 overflow-hidden">
            <a href="{{ route('threads.show', $thread) }}"
                class="hover:text-zinc-300 block whitespace-nowrap overflow-hidden text-ellipsis"
                hx-get="{{ route('threads.show', $thread) }}"
                hx-target="#main-content"
                hx-push-url="true"
                title="{{ $thread->title }}">
                {{ $thread->title }}
            </a>
        </li>
        @empty
        <li>No recent threads</li>
        @endforelse
    </ul>
            -->
</div>
