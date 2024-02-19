@props(['type' => 'text', 'className' => '', 'hasError' => false])

<input autocomplete="off" spellcheck="false" type="{{ $type }}" {{ $attributes->merge([
    'class' => "flex h-[48px] w-full rounded-md border border-2 bg-transparent p-3 text-[16px] placeholder:text-darkgray focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white disabled:cursor-not-allowed disabled:opacity-50 " . ($hasError ? 'border-red' : 'border-gray') . " $className"
]) }} />
