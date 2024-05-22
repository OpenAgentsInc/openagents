@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <x-htmx.sidebar/>
        <div class="flex flex-col w-full">

            {{--            <div hx-ext="sse" sse-connect="/message-stream" sse-swap="message">--}}
            {{--            </div>--}}

            {{--            <div hx-ext="sse" sse-connect="/message-stream">--}}
            {{--                <div sse-swap="TestStream"></div>--}}
            {{--                <div sse-swap="TestStream2"></div>--}}
            {{--                <div sse-swap="messagestreamtest"></div>--}}
            {{--            </div>--}}

            <div hx-ext="sse" sse-connect="/message-stream">
                <div sse-swap="messagestreamtest"></div>
            </div>

            {{--            <div hx-ext="sse" sse-connect="/message-stream3">--}}
            {{--                <div sse-swap="TestStream3"></div>--}}
            {{--            </div>--}}

            <div class="flex-grow w-full overflow-y-auto flex flex-col items-center">
                <x-htmx.messages-list :messages="$messages ?? null" :thread="$thread ?? null"/>
            </div>
            <x-htmx.chatbar/>
            <x-htmx.messages-remaining/>
        </div>
    </div>
</x-htmx-layout>
