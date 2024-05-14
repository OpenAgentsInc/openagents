<div class="border border-offblack rounded-lg p-4 flex flex-col leading-normal">
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
    <div class="flex-grow">
        <p class="text-gray text-base">{{ $agent['about'] }}</p>
    </div>

    <div class="mt-4 flex justify-end items-center">
        <div class="flex items-center">
            <i class="fas fa-comments mr-2"></i>
            <span>{{ $agent->thread_count }}</span>
        </div>
        <div class="flex items-center">
            <i class="fas fa-user mr-2"></i>
            <span>{{ $agent->unique_users_count }}</span>
        </div>
    </div>
</div>
