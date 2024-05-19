<div hx-get="{{ route('threads.index') }}" hx-trigger="load" hx-target="#threads-list">
    <ol id="threads-list">
        @foreach($threads as $thread)
            <a href="/chat/{{ $thread->id }}" class="flex items-center gap-2 py-1">
                <div class="relative grow overflow-hidden whitespace-nowrap">
                    {{ $thread->title }}
                </div>
            </a>
        @endforeach
    </ol>
</div>