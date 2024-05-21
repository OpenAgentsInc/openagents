@props(['messages' => [], 'thread' => null])

<div id="main-chat" class="w-full h-full prose prose-invert messages max-w-4xl">
    <div class="p-4 space-y-4" id="message-container">
        @if($thread && $messages)
            {{--            <h2 class="text-lg font-bold">{{ $thread->title }}</h2>--}}
            <ul class="space-y-2">
                @foreach($messages as $message)
                    <x-htmx.message :message="$message"/>
                @endforeach
            </ul>
        @else
            <p>Select a thread to view messages.</p>
        @endif
    </div>
</div>