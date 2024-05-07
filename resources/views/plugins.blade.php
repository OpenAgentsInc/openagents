<x-static-layout>
    <div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
        <div class="flex flex-col items-center pt-6 sm:pt-0">
            <a href="/" wire:navigate class="mt-12">
                <x-icon.logo class="w-20 h-20 text-white"/>
            </a>

            <a href="/" wire:navigate>
                <h3 class="text-[16px] fixed top-[18px] left-[24px] text-gray"> &larr; Back to chat</h3>
            </a>

            <h1 class="mt-12 text-center">Plugin Registry</h1>

            <div class="mt-0 max-w-2xl">

                @foreach ($plugins as $plugin)
                    <div class="m-16">
                        <x-pane title="{{$plugin['name']}}">
                            <p>
                                <b> Description: </b>
                                <i>"{{$plugin['description']}}"</i>
                            </p>

                            <a href="{{$plugin['url']}}" target="_blank">
                                <b>Source code: </b>
                                {{$plugin['url']}}
                            </a>

                            <p>
                                <b>Author: </b>
                                {{$plugin['author']}}
                            </p>
                        </x-pane>
                    </div>
                @endforeach
            </div>

        </div>
    </div>
</x-static-layout>