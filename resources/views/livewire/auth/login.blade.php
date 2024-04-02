<div class="bg-black text-white p-[32px]">
    <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">Log in</h2>

    <form class="p-4">
        <div>
            <x-label for="email" value="{{ __('Email') }}"/>
            <x-chat.input id="email" class="block mt-[6px] w-full" type="email" name="email" :value="old('email')"
                          required autofocus autocomplete="username" placeholder="Enter email"/>
        </div>
        <div class="mt-6">
            <div class="flex flex-row justify-between">
                <x-label for="password" value="{{ __('Password') }}"/>
                <a class="text-sm text-gray cursor-pointer"
                   wire:click="$dispatch('openModal', { component: 'auth.forget-password' })"
                >Forgot?</a>
            </div>
            <x-chat.input id=" password" class="block mt-[6px] w-full" type="password" name="password" required
                          autocomplete="current-password" placeholder="Enter password"/>
        </div>

        <div class="mt-6">
            <x-button class="w-full text-center justify-center gap-2 py-2" wire:click='showLogin()'>
                Log in
            </x-button>

            <div class="py-3 my-2 flex flex-col items-center text-sm text-[#777A82]">
                or
            </div>

            <x-secondary-button class="w-full flex justify-center gap-2 mb-0">
                <x-icon.google class="h-5 w-5"></x-icon.google>
                <span class="text-sm md:text-md">Log in with Google</span>
            </x-secondary-button>
        </div>
    </form>
</div>
