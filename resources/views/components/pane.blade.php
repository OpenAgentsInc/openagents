<!-- resources/views/components/pane.blade.php -->
@props(['title', 'subtitle' => null, 'borderColor' => 'border-text'])

<div class="bg-black text-text font-mono w-full max-w-[1050px] mx-auto">
    <div class="{{ $borderColor }} border-2 relative pt-[18px] px-[16px] pb-[14px] mb-5">
        <div class="select-none flex justify-between items-center">
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
