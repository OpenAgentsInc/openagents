<div class="bg-black text-white p-[32px]">
    <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">Log in</h2>

    <form wire:submit.prevent="login" class="p-4">
        <div>
            <x-label for="email" value="{{ __('Email') }}"/>
            <x-chat.input wire:model='email' id="email"
                          class="block mt-[6px] w-full  @error('email') border border-red-500 @enderror" type="email"
                          name="email" :value="old('email')"
                          required autofocus autocomplete="username" placeholder="Enter email"/>
            @error('email') <span class="text-red-500">{{ $message }}</span> @enderror
        </div>
        <div class="mt-6">
            <div class="flex flex-row justify-between">
                <x-label for="password" value="{{ __('Password') }}"/>
                <a class="text-sm text-gray cursor-pointer"
                   wire:click="$dispatch('openModal', { component: 'auth.forget-password' })"
                >Forgot?</a>
            </div>
            <div>
                <x-chat.input wire:model='password' id="password" class="block mt-[6px] w-full" type="password"
                              name="password" required
                              autocomplete="current-password" placeholder="Enter password"/>
                @error('password') <span class="text-red-500">{{ $message }}</span> @enderror
            </div>
        </div>

        <div class="mt-6">
            <x-button class="w-full text-center justify-center gap-2 py-2">
                Log in
            </x-button>

            <div class="py-3 my-2 flex flex-col items-center text-sm text-[#777A82]">
                or
            </div>

            <a href="/login/x">
                <x-secondary-button class="w-full flex justify-center gap-2 mb-0 h-[44px]">
                    <x-icon.x class="h-5 w-5"></x-icon.x>
                    <span class="">Log in with X</span>
                </x-secondary-button>
            </a>
        </div>
    </form>
</div>
