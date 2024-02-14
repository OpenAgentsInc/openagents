<div class="message-output">
    @if(isset($textResponse))
        <div class="flex items-start justify-start">
            <div class="w-full bg-darkgray text-white p-3 rounded-lg overflow-y-auto">
                {{ $textResponse }}
            </div>
        </div>
    @endif
</div>
