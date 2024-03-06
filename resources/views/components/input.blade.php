@props(['type' => 'text', 'className' => '', 'hasError' => false, 'showIcon' => false, 'iconName' => ''])

<div class="relative">
    <input autocomplete="off" spellcheck="false" type="{{ $type }}" {{ $attributes->merge([
        'class' => "flex h-[48px] w-full rounded-md border border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-darkgray focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white disabled:cursor-not-allowed disabled:opacity-50 " . ($hasError ? 'border-red' : 'border-gray') . " $className"
    ]) }} />

    @if($showIcon && $iconName)
        <div class="absolute md:bottom-[10px] md:right-[10px] bg-white text-black shadow hover:bg-white/90 rounded">
            <x-icon :name="$iconName" class="w-[24px] h-[24px] m-0.5 flex flex-col justify-center items-center"/>
        </div>
    @endif
</div>
