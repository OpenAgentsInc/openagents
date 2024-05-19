@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <div class="w-[260px] border-r border-darkgray p-4">
            <div hx-get="{{ route('threads.index') }}" hx-trigger="load"></div>
        </div>
        <div class="w-3/4">
            <div id="main-chat" class="h-full overflow-y-auto">
                <x-htmx.messages-list/>
            </div>
        </div>
    </div>
</x-htmx-layout>
