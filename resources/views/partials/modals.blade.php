 <!-- login modal -->
 <div x-data="{ open: false }" @open-login-modal.window="open = true" @close-login-modal.window="open = false">
     <!-- Trigger button -->
     <!-- Modal -->
     <div x-show="open" class="fixed inset-0 flex items-center justify-center  bg-black/90 w-full h-full z-[50]">


         <!-- Overlay -->
         <div class="fixed bg-black/20 rounded-lg p-6  w-full max-w-full shadow-lg transform transition-all duration-300 z-[5]" x-show.transition.opacity="open"></div>

         <div @click.away="$dispatch('close-login-modal')" class="fixed border  border-[#3C3E42] bg-black rounded-lg p-6  max-w-md   shadow-lg transform transition-all duration-300 z-[5000]" x-show.transition.opacity="open">
             <!-- Modal Header -->

             <div class="flex justify-end items-center  pb-4">
                 {{-- <h2 class="text-2xl font-semibold">Get Started</h2> --}}
                 <button @click="open = false" class="text-gray-500 hover:text-gray-700 focus:outline-none">
                     <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="feather feather-x">
                         <line x1="18" y1="6" x2="6" y2="18"></line>
                         <line x1="6" y1="6" x2="18" y2="18"></line>
                     </svg>
                 </button>
             </div>

             <div class="">
                 <h2 class="block text-xl text-center font-bold text-gray-800 dark:text-gray-200">Sign In</h2>
             </div>

             <div class="p-4 sm:p-7">

                 <div class="mb-4">
                     <x-input id="email" class="block mt-1 w-full" type="email" name="email" :value="old('email')" required autofocus autocomplete="username" placeholder="email" />
                 </div>

                 <div class="flex justify-end items-center">

                     <a @click="$dispatch('open-resetpassword-modal')" @click="$dispatch('close-login-modal')" class="text-sm text-gray decoration-2 hover:underline hover:text-white font-medium focus:outline-none focus:ring-1 focus:ring-gray-600" href="#">Forgot password?</a>
                 </div>

                 <div class="mb-4">
                     <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus autocomplete="password" placeholder="password" />
                 </div>


                 <div class="mt-5">
                     {{-- <a class="w-full py-3 px-4 inline-flex justify-center items-center gap-x-2 text-sm font-medium rounded-lg border border-gray-200 bg-white text-gray-800 shadow-sm hover:bg-gray-50 disabled:opacity-50 disabled:pointer-events-none dark:bg-slate-900 dark:border-gray-700 dark:text-white dark:hover:bg-gray-800 dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" href="#">

                        </a> --}}
                     <x-button class="w-full flex justify-center gap-2 hover:bg-gray">
                         <x-icon.agent class="h-5 w-5 text-black"></x-icon.agent>
                         Sign In
                     </x-button>

                     <div class="py-3 flex items-center text-xs text-gray-400 uppercase before:flex-[1_1_0%] before:border-t before:border-gray-200 before:me-6 after:flex-[1_1_0%] after:border-t after:border-gray-200 after:ms-6 dark:text-gray-500 dark:before:border-gray-600 dark:after:border-gray-600">Or</div>


                     <x-secondary-button class="w-full flex justify-center gap-2">
                         <x-icon.google class="h-5 w-5"></x-icon.google>
                         Continue with Google
                     </x-secondary-button>

                 </div>

                 <div class="text-center">
                     <p class="mt-2 text-sm text-gray">
                         Do not have an account?
                         <a @click="$dispatch('open-register-modal')" id="hs-dropdown-with-header" type="button" class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" href="#">
                             Create an account
                         </a>

                     </p>
                 </div>
             </div>





         </div>
     </div>
 </div>

 <!-- registration modal -->
 <div x-data="{ open: false }" @open-register-modal.window="open = true" @close-register-modal.window="open = false">
     <!-- Trigger button -->
     <!-- Modal -->
     <div x-show="open" @click.away="$dispatch('close-register-modal')" class="fixed inset-0 flex items-center justify-center  bg-black/90 w-full h-full z-[500]">


         <!-- Overlay -->
         <div class="fixed bg-black/20 rounded-lg p-6  w-full max-w-full shadow-lg transform transition-all duration-300 z-[5]" x-show.transition.opacity="open"></div>

         <div class="fixed border border-[#3C3E42] bg-black rounded-lg p-6  max-w-md   shadow-lg transform transition-all duration-300 z-[5000]" x-show.transition.opacity="open">
             <!-- Modal Header -->

             <div class="flex justify-end items-center  pb-4">
                 {{-- <h2 class="text-2xl font-semibold">Get Started</h2> --}}
                 <button @click="open = false" class="text-gray-500 hover:text-gray-700 focus:outline-none">
                     <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="feather feather-x">
                         <line x1="18" y1="6" x2="6" y2="18"></line>
                         <line x1="6" y1="6" x2="18" y2="18"></line>
                     </svg>
                 </button>
             </div>

             @livewire('auth.register')





         </div>
     </div>
 </div>


 <!-- Forgot Password modal -->
 <div x-data="{ open: false }" @open-resetpassword-modal.window="open = true" @close-resetpassword-modal.window="open = false">
     <!-- Trigger button -->
     <!-- Modal -->
     <div x-show="open" @click.away="$dispatch('close-resetpassword-modal')" class="fixed inset-0 flex items-center justify-center  bg-black/90 w-full h-full z-[500]">


         <!-- Overlay -->
         <div class="fixed bg-black/20 rounded-lg p-6  w-full max-w-full shadow-lg transform transition-all duration-300 z-[5]" x-show.transition.opacity="open"></div>

         <div class="fixed border border-[#3C3E42] bg-black rounded-lg p-6  max-w-md   shadow-lg transform transition-all duration-300 z-[5000]" x-show.transition.opacity="open">
             <!-- Modal Header -->

             <div class="flex justify-end items-center  pb-4">
                 {{-- <h2 class="text-2xl font-semibold">Get Started</h2> --}}
                 <button @click="open = false" class="text-gray-500 hover:text-gray-700 focus:outline-none">
                     <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="feather feather-x">
                         <line x1="18" y1="6" x2="6" y2="18"></line>
                         <line x1="6" y1="6" x2="18" y2="18"></line>
                     </svg>
                 </button>
             </div>

             <div class="">
                 <h2 class="block text-xl text-center font-bold text-gray-800 dark:text-gray-200">Reset Password</h2>
             </div>



             <div class="p-4 sm:p-7">

                 <div class=" flex justify-center">
                     <span class="mb-4 inline-flex justify-center items-center  rounded-full">
                         <x-icon.padlock class="w-[100px] h-[100px]">
                         </x-icon.padlock>
                     </span>
                 </div>

                 <div class="mb-4">
                     <x-input id="email" class="block mt-1 w-full" type="email" name="email" :value="old('email')" required autofocus autocomplete="username" placeholder="email" />
                 </div>


                 <div class="mt-5">
                     {{-- <a class="w-full py-3 px-4 inline-flex justify-center items-center gap-x-2 text-sm font-medium rounded-lg border border-gray-200 bg-white text-gray-800 shadow-sm hover:bg-gray-50 disabled:opacity-50 disabled:pointer-events-none dark:bg-slate-900 dark:border-gray-700 dark:text-white dark:hover:bg-gray-800 dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" href="#">

                        </a> --}}
                     <x-button class="w-full flex justify-center gap-2 hover:bg-gray">
                         <x-icon.agent class="h-5 w-5 text-black"></x-icon.agent>
                         Reset Account
                     </x-button>



                 </div>

                 <div class="text-center">
                     <p class="mt-2 text-sm text-gray">
                         Do you have an account?
                         <a @click="$dispatch('close-resetpassword-modal')" class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" href="#">
                             Login
                         </a>


                     </p>
                 </div>
             </div>





         </div>
     </div>
 </div>
