@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <x-htmx.sidebar/>
        <div class="flex flex-col w-full">

            <div hx-ext="sse" sse-connect="/message-stream" sse-swap="message">
                Contents of this box will be updated in real time
                with every SSE message received from the chatroom.
            </div>

            <div class="flex-grow w-full overflow-y-auto flex flex-col items-center">
                <x-htmx.messages-list :messages="$messages ?? null" :thread="$thread ?? null"/>
            </div>
            <x-htmx.chatbar/>
            <x-htmx.messages-remaining/>
        </div>
    </div>
</x-htmx-layout>
