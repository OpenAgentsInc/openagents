@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <div class="w-[260px] border-r border-darkgray p-4">
            <div hx-get="{{ route('threads.index') }}" hx-trigger="load" hx-target="#threads-list">
                <ol id="threads-list">
                    @foreach($threads as $thread)
                        <button class="flex items-center gap-2 py-1"
                                hx-get="/threads/{{ $thread->id }}"
                                hx-target="#main-chat"
                                hx-swap="innerHTML"
                                hx-trigger="click">
                            <div class="relative grow overflow-hidden whitespace-nowrap">
                                {{ $thread->title }}
                            </div>
                        </button>
                    @endforeach
                </ol>
            </div>
        </div>
        <div class="w-3/4">
            <div id="main-chat" class="h-full overflow-y-auto">
                <x-htmx.messages-list/>
            </div>
        </div>
    </div>
</x-htmx-layout>
