<div class="w-full text-lightgray">
    <div class="px-4 py-2 justify-center text-base md:gap-6 m-auto">
        <div
            class="flex flex-1 text-base mx-auto gap-3 md:px-5 lg:px-1 xl:px-5 md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
            <div class="flex-shrink-0 flex flex-col relative items-end">
                <div class="mt-1 h-6 w-6 items-center justify-center bg-darkgray rounded-full">
                </div>
            </div>
            <div class="relative flex w-full flex-col">
                <span class="font-semibold select-none text-white">{{ $author }}</span>
                <div class="flex-col gap-1 md:gap-3">
                    <div class="flex flex-grow flex-col max-w-full">
                        <span class="text-sm">{{ $message }}</span>
                    </div>
                    <div class="flex justify-start gap-3 empty:hidden">
                        <div
                            class="text-gray-400 flex self-end lg:self-center justify-center lg:justify-start mt-0 -ml-1 h-7 visible">
                        </div>
                    </div>
                </div>
            </div>

        </div>
    </div>
</div>

<!--
<div class="flex flex-row justify-start px-4 py-2">
    <div class="bg-black rounded-lg px-4 py-2 max-w-xs">
        <p class="text-sm">{{ $author }}</p>
        <p class="text-sm">{{ $message }}</p>
    </div>
</div>
-->
