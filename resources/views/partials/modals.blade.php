 <!-- login modal -->
 <div x-data="{ open: false }" @open-login-modal.window="open = true" @close-login-modal.window="open = false">
     <!-- Trigger button -->
     <!-- Modal -->
     <div x-show="open" class="fixed inset-0 flex items-center justify-center  bg-black/90 w-full h-full z-[50]">


         <!-- Overlay -->
         <div class="fixed bg-black/20 rounded-lg p-6  w-full max-w-full shadow-lg transform transition-all duration-300 z-[5]" x-show.transition.opacity="open"></div>
         <div @click.away="$dispatch('close-login-modal')" class="fixed border  border-[#3C3E42] bg-black rounded-lg p-6  max-w-md   shadow-lg transform transition-all duration-300 z-[5000]" x-show.transition.opacity="open">
         @livewire('auth.login')
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



             @livewire('auth.register')





         </div>
     </div>
 </div>


 <!-- Forgot Password modal -->
 <div x-data="{ open: false }" @open-resetpassword-modal.window="open = true" @close-resetpassword-modal.window="open = false">
     <!-- Trigger button -->

     <!-- Modal -->

     <div x-show="open" class="fixed inset-0 flex items-center justify-center  bg-black/90 w-full h-full z-[50]">


        <!-- Overlay -->
        <div class="fixed bg-black/20 rounded-lg p-6  w-full max-w-full shadow-lg transform transition-all duration-300 z-[5]" x-show.transition.opacity="open"></div>
        <div @click.away="$dispatch('close-resetpassword-modal')" class="fixed border  border-[#3C3E42] bg-black rounded-lg p-6  max-w-md   shadow-lg transform transition-all duration-300 z-[5000]" x-show.transition.opacity="open">
            @livewire('auth.forget-password')
        </div>


    </div>
 </div>
