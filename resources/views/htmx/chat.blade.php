@props(['threads' => []])

<x-htmx-layout>
    <div class="flex h-full">
        <x-htmx.sidebar/>
        <div class="flex flex-col w-full">

            <div hx-ext="sse" sse-connect="/stream">
                <div sse-swap="message" hx-swap="beforeend"></div>
            </div>

            <div class=" flex-grow w-full overflow-y-auto flex flex-col items-center">
                <x-htmx.messages-list :messages="$messages ?? null" :thread="$thread ?? null"/>
            </div>
            <x-htmx.chatbar/>
            <x-htmx.messages-remaining/>
        </div>
        <x-htmx.bonus-widget/>
    </div>

    <script>
        {{--var source = new EventSource("{{ URL('/stream') }}");--}}

        {{--source.onopen = function (event) {--}}
        {{--    console.log("Open:", event)--}}
        {{--}--}}

        {{--source.onmessage = function (event) {--}}
        {{--    console.log("Message:", event)--}}
        {{--}--}}

        {{--source.onerror = function (event) {--}}
        {{--    console.log("Error:", event)--}}
        {{--}--}}

        {{--source.onmessage = function (event) {--}}
        {{--    console.log(event)--}}
        {{--}--}}
    </script>
</x-htmx-layout>
