<div class="z-[-1] w-full text-lightgray">
    <div class="px-1 justify-center text-base md:gap-4 m-auto">
        <div class="flex flex-1 text-base mx-auto gap-3 md:px-5 lg:px-1 xl:px-5 md:max-w-3xl lg:max-w-[800px]">
            <div class="flex-shrink-0 flex flex-col relative items-end not-prose">
                @if ($author === 'You')
                    <div class="m-[2px] w-[28px] p-[2px] border border-darkgray rounded">
                        <x-icon name="user" class=""/>
                    </div>
                @else

                    <!-- if the message author is agent, show their image here -->
                    @if ($image)
                        <div class="m-[1px] w-[28px] p-[2px] border border-darkgray rounded">
                            <img src="{{ $image }}" alt="Image" />
                        </div>
                    @else
                        <div class="m-[1px] w-[28px] p-[5px] border border-darkgray rounded">
                            <x-icon name="logo" class=""/>
                        </div>
                    @endif

                @endif
            </div>
            <div class="relative flex w-full flex-col items-start"> <!-- Add "items-start" here -->
                <span class="mb-1 font-semibold select-none text-white">{{ $author }}</span>
                <div class="flex-col">
                    <div class="-mt-4 flex flex-grow flex-col max-w-[936px]">

                        @if(substr($message, 0, 11) === 'data:image/')
                            <img class="mt-6" src="{{ $message }}" alt="Embedded Image">
                        @else
                            <x-markdown
                                    class="text-md text-text markdown-content {{ $promptClass }}">{!! $message !!}</x-markdown>
                            <div class="dot-flashing opacity-0"></div>
                        @endif

                    </div>

                </div>
            </div>
        </div>
    </div>
</div>
