@props(['threads' => []])

<div>
    @foreach($threads as $thread)
        <button class="flex items-center gap-2 py-1"
                hx-get="/threads/{{ $thread->id }}"
                hx-target="#main-chat"
                hx-swap="innerHTML"
                hx-trigger="click"
        >
            <div class="relative grow overflow-hidden whitespace-nowrap">
                {{ $thread->title }}
            </div>
        </button>
    @endforeach
</div>
