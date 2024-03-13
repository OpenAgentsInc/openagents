@props([
    'type' => 'text',
    'className' => '',
    'hasError' => false,
    'showIcon' => false,
    'iconName' => '',
])

<div class="relative" x-data="{ inputVal: '', isDisabled: true }">
    <input
            autocomplete="off"
            spellcheck="false"
            type="{{ $type }}"
            @input="inputVal = $event.target.value; isDisabled = inputVal.length === 0"
            {{ $attributes->merge([
                'class' => "flex h-[48px] w-full rounded-md border border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white " . ($hasError ? 'border-red' : 'border-[#3D3E42]') . " $className"
            ]) }}
    />

    @if($showIcon && $iconName)
        <button
                :disabled="isDisabled"
                class="absolute bottom-[10px] right-[10px] text-black shadow rounded"
                :class="isDisabled ? 'bg-gray' : 'bg-white hover:bg-white/90'"
        >
            <x-icon :name="$iconName" class="w-[24px] h-[24px] m-0.5 flex flex-col justify-center items-center"/>
        </button>
    @endif
</div>
