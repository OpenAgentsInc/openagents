<div class="bg-black text-white p-[32px]">

    @if(!$this->verification)

        <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">{{$this->show ? 'Create Password' : 'Sign up'}}</h2>

    @else
        <div class="py-4 mt-2">
            <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">Verify your email</h2>
        </div>
    @endif

    @if(!$this->show)
        {{-- Step 1 input email --}}
        <div class="p-4 pb-0">
            <div class="mb-4">
                <x-label for="register-email" value="{{ __('Email') }}"/>
                <x-input wire:model='email' id="register-email" class="block mt-[6px] w-full" type="email" name="email"
                         :value="old('email')" required autofocus autocomplete="username" placeholder="Enter email"/>
            </div>

            <div class="mt-6 mb-3">
                <x-button class="w-full flex justify-center gap-2 text-center" wire:click='showpassword()'>
                    Continue
                </x-button>

                <div class="mt-2 py-3 flex flex-col items-center text-sm text-[#777A82]">
                    or
                </div>

                <a href="/login/x">
                    <x-secondary-button class="w-full flex justify-center gap-2 my-2">
                        <x-icon.x class="h-5 w-5"></x-icon.x>
                        <span class="text-sm md:text-md">Continue with X</span>
                    </x-secondary-button>
                </a>
            </div>

            <div class="text-center">
                <p class="mt-2 mb-0 text-sm text-gray">
                    By continuing you agree to our
                    <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="/terms" target="_blank">
                        Terms of Service
                    </a>
                    and
                    <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="/privacy" target="_blank">
                        Privacy Policy.
                    </a>
                </p>
            </div>
        </div>
    @endif
    @if ($this->show && !$this->verification)
        {{-- Step 2 Input password --}}
        <form wire:submit.prevent='create'>
            <div class="p-2">
                <div class="mb-6">
                    <x-input id="password" wire:model='password' class="block mt-1 w-full" type="password"
                             name="password" required autofocus placeholder="Enter password..."/>
                    @error('password') <span class="text-red-500">{{ $message }}</span> @enderror
                </div>

                <div class="mb-6">
                    <x-input id="password" wire:model='password_confirmation' class="block mt-1 w-full" type="password"
                             name="password_confirmation" required autofocus placeholder="Confirm password..."/>
                    @error('password_confirmation') <span class="text-red-500">{{ $message }}</span> @enderror
                </div>

                <div class="mt-8">
                    <x-button class="w-full flex justify-center text-center gap-2">
                        Create Password
                    </x-button>
                </div>
            </div>
        </form>
    @endif
    @if ($this->show && $this->verification)
        {{-- Send Verification --}}
        <div class="p-4 sm:p-7">
            <div class="text-center">
                <p class="mt-2 text-sm md:text-md text-gray">
                    We sent a verification link to <span class="text-white">satoshi@nakamoto.com</span>
                </p>
                <p>
                    <a role="button" wire:click='resend()'
                       class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="#">
                        Resend Email
                    </a>
                </p>
            </div>
        </div>
    @endif
</div>
