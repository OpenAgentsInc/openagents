@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <x-htmx.sidebar/>
        <div class="flex flex-col w-full">

            <div hx-ext="sse" sse-connect="/stream">
                <p>Demo Event 1:</p>
                <div sse-swap="event1"></div>

                <p>Demo Event 2:</p>
                <div sse-swap="event2"></div>

                <p>Messages Catch-all:</p>
                <div sse-swap="message" hx-swap="beforeend"></div>
            </div>

            {{--            <div hx-ext="sse" sse-connect="/message-stream" sse-swap="message">--}}
            {{--            </div>--}}

            {{--            <div hx-ext="sse" sse-connect="/event-stream" sse-swap="message">--}}
            {{--                Contents of this box will be updated in real time with every SSE message received.--}}
            {{--            </div>--}}

            {{--            <div id="chat-messages" hx-ext="sse" sse-connect="/stream" sse-swap="message" hx-swap="beforeend">--}}
            {{--                <p>Chat messages will appear here in real-time.</p>--}}
            {{--            </div>--}}

            {{--            <div hx-ext="sse" sse-connect="/message-stream">--}}
            {{--                <div sse-swap="TestStream"></div>--}}
            {{--                <div sse-swap="TestStream2"></div>--}}
            {{--                <div sse-swap="messagestreamtest"></div>--}}
            {{--            </div>--}}

            {{--            <div hx-ext="sse" sse-connect="/message-stream" sse-swap="outerHTML">--}}
            {{--                <div id="messagestreamtest"></div>--}}
            {{--            </div>--}}

            {{--            <div hx-ext="sse" sse-connect="/message-stream" sse-swap="messagestreamtest"></div>--}}

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
