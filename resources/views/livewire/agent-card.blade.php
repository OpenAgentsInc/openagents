<a class="select-auto pointer-events-auto border border-offblack hover:border-darkgray rounded-lg p-4 flex flex-col leading-normal"
   href="/chat?agent={{ $agent->id }}"
>
    <div class="mt-1 mb-3 w-[20px] h-[20px] sm:w-[60px] sm:h-[60px]">
        <img src="{{ $agent->image_url }}" alt="Agent" class="w-full h-full rounded">
    </div>

    <div class="font-bold text-xl">{{ $agent['name'] }}</div>
    <p class="mb-0 text-sm text-gray leading-none">{{ $agent->sats_per_message }} sats per message</p>
    <div class="flex items-center">
        <div class="text-xs">
            <p class="text-sm text-gray leading-none">From: {{ $agent->user->name }}</p>
        </div>
        <img class="w-5 h-5 rounded-full mx-4" src="{{ $agent->user->profile_photo_path }}"
             alt="Avatar of {{ $agent->user->name }}">
    </div>
    <div class="flex-grow">
        <p class="text-sm text-text my-1">{{ $agent['about'] }}</p>
    </div>


    <div class="text-gray mt-4 gap-x-6 flex justify-end items-center">
        <div class="flex items-center">
            <x-icon.bitcoin class="w-4 h-4 mr-1"/>
            <span>{{ $agent->sats_earned ?? 0 }}</span>
        </div>
        <div class="flex items-center">
            <x-icon.chats class="w-4 h-4 mr-1"/>
            <span>{{ $agent->thread_count }}</span>
        </div>
        <div class="flex items-center">
            <x-icon.user class="w-4 h-4 mr-1"/>
            <span>{{ $agent->unique_users_count }}</span>
        </div>
    </div>
</a>
