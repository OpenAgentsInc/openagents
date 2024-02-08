@props(['className' => ''])

    <textarea autocomplete="off" spellcheck="false" {{ $attributes->merge([
    'class' => "mt-1 flex min-h-[60px] w-full rounded-md border border-input bg-transparent p-3 text-sm placeholder:text-gray focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50 $className"
]) }}>{{ $slot }}</textarea>
