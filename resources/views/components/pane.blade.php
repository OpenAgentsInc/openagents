@props(['title', 'subtitle' => null])

<div class="bg-black text-white font-mono w-full max-w-[1050px] mx-auto">
    <div class="border-2 border-white relative pt-[18px] px-[16px] pb-[14px] mb-5">
        <div class="flex justify-between items-center">
            <div class="absolute text-lg font-bold top-[-15px] left-[6px] bg-black px-2.5">{{ $title }}</div>
            @if ($subtitle)
                <div class="text-text text-sm absolute top-[-12px] right-[6px] bg-black px-2.5">{{ $subtitle }}</div>
            @endif
        </div>
        <div class="pt-[2px]">
            {{ $slot }}
        </div>
    </div>
</div>