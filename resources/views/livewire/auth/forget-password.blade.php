<div class="bg-black text-white p-[32px]">
    <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">{{$this->show ? 'Reset password' : 'Forgot password'}}</h2>

    @if(!$this->show)
        <div class="p-4 pb-0">
            <div class="mt-2 mb-4">
                <x-label for="email" value="{{ __('Email') }}"/>
                <x-input id="forget-password-email" class="block mt-[6px] w-full" type="email" name="email"
                         :value="old('email')" required autofocus autocomplete="username" placeholder="Enter email"/>
            </div>
            <div class="my-5">
                <x-button class="w-full flex justify-center gap-2 " wire:click='sendResetLink()'>
                    Reset password
                </x-button>
            </div>

            <div class="text-center">
                <p class="mt-4 mb-0 text-sm text-gray">
                    You will receive a password reset link if you have an account with us.
                </p>
            </div>
        </div>
    @elseif ($this->show)
        <div class="p-4 pb-0">
            <div class="text-center">
                <p class="mt-2 text-sm md:text-md text-gray">
                    We sent a reset link to <span class="text-white">satoshi@nakamoto.com</span>.
                </p>
                <p class="mb-0">
                    <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="#">
                        Resend Email
                    </a>
                </p>
            </div>
        </div>
    @endif
</div>
