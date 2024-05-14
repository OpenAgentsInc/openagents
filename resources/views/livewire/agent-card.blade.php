<div class="border border-offblack rounded-lg p-4 flex flex-col justify-between leading-normal">
    <div class="mt-1 mb-4 w-[20px] h-[20px] sm:w-[60px] sm:h-[60px]">
        <img src="{{ $agent->image_url }}" alt="Agent" class="w-full h-full rounded">
    </div>

    <div class="font-bold text-xl">{{ $agent['name'] }}</div>
    <div class="flex items-center">
        <div class="text-xs">
            <p class="text-gray leading-none">From: {{ $agent->user->username }}</p>
        </div>
        <img class="w-6 h-6 rounded-full mx-4" src="{{ $agent->user->profile_photo_path }}"
             alt="Avatar of {{ $agent->user->username }}">
    </div>
    <p class="text-gray text-base">{{ $agent['about'] }}</p>
</div>


{{--<a class="pointer-events-auto select-auto" href="/chat?agent={{ $agent['id'] }}" wire:navigate>--}}
{{--    <div class="p-4 rounded-lg relative">--}}
{{--        <div class="flex justify-between">--}}
{{--            <div class="flex">--}}
{{--                <div class="mt-1 w-[20px] h-[20px] sm:w-[60px] sm:h-[60px]">--}}
{{--                    <img src="{{ $agent->image_url }}" alt="Agent" class="w-full h-full rounded">--}}
{{--                </div>--}}
{{--                <div class="flex-1 pl-4">--}}
{{--                    <h4 class="text-lg font-bold">{{ $agent['name'] }}</h4>--}}
{{--                    <span class="text-gray">{{ $agent['about'] }}</span>--}}
{{--                    <p>By: {{ $agent->creator_username }}</p>--}}
{{--                </div>--}}
{{--            </div>--}}
{{--            <div class="absolute bottom-0 right-0 p-4">--}}
{{--                <span class="inline-flex items-center">--}}
{{--                    <i class="fas fa-comments"></i> {{ $agent->thread_count }}--}}
{{--                </span>--}}
{{--                <span class="inline-flex items-center ml-2">--}}
{{--                    <i class="fas fa-user"></i> {{ $agent->unique_users_count }}--}}
{{--                </span>--}}
{{--            </div>--}}
{{--        </div>--}}
{{--    </div>--}}
{{--</a>--}}
