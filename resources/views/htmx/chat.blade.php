@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <x-htmx.sidebar/>
        <div class="flex flex-col overflow-hidden w-full">
            <div class="flex-grow w-full overflow-y-auto flex flex-col items-center">
                <x-htmx.messages-list :messages="$messages ?? null" :thread="$thread ?? null"/>
            </div>
            <x-htmx.chatbar/>
        </div>
    </div>
</x-htmx-layout>
