<div class="group relative mb-4 flex items-start">
    <div class="flex size-7 shrink-0 select-none items-center justify-center rounded-md border border-zinc-700 p-1 shadow {{ $message['user_id'] !== null ? 'bg-background' : 'bg-black text-white' }}">
        @if($message['user_id'] !== null || !isset($message['model']))
        <!-- Replace with your user icon component or SVG -->
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M20 21V19C20 17.9391 19.5786 16.9217 18.8284 16.1716C18.0783 15.4214 17.0609 15 16 15H8C6.93913 15 5.92172 15.4214 5.17157 16.1716C4.42143 16.9217 4 17.9391 4 19V21" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
            <path d="M12 11C14.2091 11 16 9.20914 16 7C16 4.79086 14.2091 3 12 3C9.79086 3 8 4.79086 8 7C8 9.20914 9.79086 11 12 11Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        @else
        <!-- Replace with your OpenAgents icon component or SVG -->
        <x-icons.openagents class="w-5 h-5" />
        @endif
    </div>
    <div class="flex-1 px-1 ml-3 space-y-2 overflow-hidden">
        @if(substr($message['content'], 0, 11) === 'data:image/')
        <img class="mt-6" src="{{ $message['content'] }}" alt="Embedded Image">
        @else
        <x-markdown class="prose prose-full-width dark:prose-invert text-sm break-words leading-relaxed markdown-content whitespace-pre-wrap">{{ $message['content'] }}</x-markdown>
        @endif
    </div>
</div>
