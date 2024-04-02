<div class="fixed w-full h-[64px] px-5 py-3 flex flex-row items-center justify-between z-[300]">
    <livewire:model-selector/>

    @auth
        <div class="flex flex-row items-center">
            <x-icon.share class="cursor-pointer w-[24px] h-[24px] mr-[56px]"/>
            <a href="/logout">
                <div class="select-none cursor-pointer bg-darkgray w-[32px] h-[32px] rounded-full text-[#d7d8e5] flex items-center justify-center">
                    C
                </div>
            </a>
        </div>

    @else
        <x-login-button/>
    @endauth
</div>
