@props(['className' => ''])

    <textarea autocomplete="off" spellcheck="false" {{ $attributes->merge([
    'class' => "mt-1 flex min-h-[60px] w-full rounded-md border border-offblack placeholder:text-darkgray bg-transparent p-3 text-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-white disabled:cursor-not-allowed disabled:opacity-50 $className"
]) }}>{{ $slot }}</textarea>
