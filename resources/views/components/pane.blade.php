@props(['title'])

<div class="bg-black text-white font-mono w-full mx-auto">
    <div class="border-2 border-white relative pt-[18px] px-[16px] pb-[14px] mb-5">
        <div class="absolute font-bold top-[-13px] left-[6px] bg-black px-2.5">{{ $title }}</div>
        <div class="pt-[2px]">
            {{ $slot }}
        </div>
    </div>
</div>
