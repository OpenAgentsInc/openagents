<x-htmx-layout>
    <div class="flex flex-col w-full relative z-50 h-full">
        <div class="flex-1 overflow-y-auto">
            <div class="flex flex-col gap-2 py-3 px-1">
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
        </div>
    </div>
</x-htmx-layout>
