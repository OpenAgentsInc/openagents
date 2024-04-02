<div class="bg-black text-white p-[32px]">
    <div class="">
        <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">{{$this->show ? 'Enter Password' : 'Get started'}}</h2>
    </div>

    <div class="p-4 sm:p-7">
        <div class="mb-4">
            <x-input id="email" class="block mt-1 w-full" type="email" name="email" :value="old('email')" required autofocus autocomplete="username" placeholder="email" />
        </div>

        @if($this->show)
        <div class="mb-4">
            <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus autocomplete="password" placeholder="password" />
        </div>
        @endif

        <div class="my-5">
            <x-button class="w-full text-center justify-center gap-2 py-2" wire:click='showLogin()'>
                {{$this->show ? 'Enter Password' : 'Get started'}}
            </x-button>

            @if(!$this->show)
            <div class="py-3 my-2 flex items-center text-xs text-[#777A82] uppercase before:flex-[1_1_0%]  before:me-6 after:flex-[1_1_0%] after:ms-6 dark:text-gray-500 dark:before:border-gray-600 dark:after:border-gray-600">
                Or
            </div>

            <x-secondary-button class="w-full flex justify-center gap-2">
                <x-icon.google class="h-5 w-5"></x-icon.google>
                <span class="text-sm md:text-md">Continue with Google</span>
            </x-secondary-button>
            @endif

        </div>

        @if(!$this->show)
        <!-- the best place to add sign up -->
        <div class="text-center">
            <p class="mb-0 mt-6 text-sm text-gray">
                By continuing you agree to our
                <a target="_blank"
                   class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                   href="/terms">
                    Terms of Service
                </a>
                and
                <a target="_blank"
                   class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                   href="/privacy">
                    Privacy Policy.
                </a>
            </p>
        </div>
        @else
        <div class="text-center">
            <p class="mt-4 text-sm text-gray">
                <a role="button" wire:click="$dispatch('openModal', { component: 'auth.forget-password' })" id="hs-dropdown-with-header" type="button" class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" >
                    Forget Password
                </a>
            </p>
        </div>
        @endif

    </div>
</div>
