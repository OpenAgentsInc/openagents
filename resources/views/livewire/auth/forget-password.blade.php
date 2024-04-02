<div>


    <div class="py-4 mt-3">
        <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">{{$this->show ? 'Reset password' : 'Forgot Password'}}</h2>
    </div>

    @if(!$this->show)
        <div class="p-4 sm:p-7">
            <div class="mb-4">
                <x-input id="forget-password-email" class="block mt-1 w-full" type="email" name="email"
                         :value="old('email')" required autofocus autocomplete="username" placeholder="email"/>
            </div>
            <div class="my-5">
                <x-button class="w-full flex justify-center gap-2 " wire:click='sendResetLink()'>
                    Reset account
                </x-button>
            </div>

            <div class="text-center">
                <p class="mt-4 text-sm text-gray">
                    You will receive a password reset link if you have an
                    <span
                       class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600">
                        account
                </span>
                    with us.
                </p>
            </div>
        </div>
    @elseif ($this->show)
        <div class="p-4 sm:p-7">
            <div class="text-center">
                <p class="mt-2 text-sm md:text-md text-gray">
                    We sent a reset link to <span class="text-white">satoshi@nakamoto.com</span>.
                </p>
                <p>
                    <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="#">
                        Resend Email
                    </a>
                </p>
            </div>
        </div>
    @endif
</div>
