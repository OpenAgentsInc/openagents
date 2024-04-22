<div
        x-menu:items
        x-anchor.bottom-end.offset.3="document.getElementById($id('alpine-menu-button'))"
        class="min-w-[10rem] z-10 bg-black border border-offblack divide-y divide-offblack rounded-md shadow-lg py-1 outline-none"
        x-cloak
>
    {{ $slot }}
</div>
