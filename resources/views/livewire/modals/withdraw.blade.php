

    <form class="p-4" wire:submit.prevent="submitPaymentRequest"
    class="p-4 w-full flex-col items-center justify-center text-center">
        <div>
            <h4>Lightning Withdraw</h4>
               <p class="text-gray mt-4 text-xs">
                You can withdraw any amount up to {{ $balance_sats }} sats.
                Some fees may apply.
                </p>
            <label for="payment_request">Lightning invoice:</label>
            <x-chat.textarea type="text" id="payment_request" wire:model="payment_request"
                                required default="lnbc..."></x-chat.textarea>
            @error('payment_request') <span class="error">{{ $message }}</span> @enderror
        </div>

        <x-secondary-button class="mt-4 w-full" type="submit">Withdraw to invoice</x-secondary-button>
    </form>

