@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <div class="w-[260px] border-r border-darkgray p-4">
            <div hx-get="{{ route('threads.index') }}" hx-trigger="load"></div>
        </div>
        <div class="w-full overflow-y-auto flex flex-col justify-center items-center">
            <div id="main-chat" class="w-full h-full prose prose-invert messages max-w-4xl">
                <x-htmx.messages-list :messages="$messages ?? null" :thread="$thread ?? null"/>
            </div>
        </div>
    </div>
</x-htmx-layout>
