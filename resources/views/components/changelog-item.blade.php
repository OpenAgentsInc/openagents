@props([
    'code' => null,
    'post' => null,
])

<div class="flex items-center gap-4">
    <p class="mx-1 my-[2px] text-text">&middot; {{ $slot }}</p>
    <div class="flex items-center gap-1">
        @if ($code)
            <a href="{{ $code }}" target="_blank" class="text-sm text-gray">Code</a>
        @endif
        @if ($code && $post)
            <span class="text-sm text-gray">&middot;</span>
        @endif
        @if ($post)
            <a href="{{ $post }}" target="_blank" class="text-sm text-gray">Post</a>
        @endif
    </div>
</div>
