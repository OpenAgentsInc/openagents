@props(['type' => 'text', 'className' => ''])

<input 
autocomplete="off"
spellcheck="false"
type="{{ $type }}" {{ $attributes->merge([
    'class' => "mt-1 flex h-10 w-full rounded-md border border-gray bg-transparent p-3 text-sm placeholder:text-gray focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-white disabled:cursor-not-allowed disabled:opacity-50 $className"
]) }} />
