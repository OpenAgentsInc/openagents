<div class="p-4 md:p-12 mx-auto flex flex-col justify-center w-full items-center" x-data="{ dropdown: false }">
    <div class="w-full md:max-w-3xl md:min-w-[600px]">
        <h3 class="mb-16 font-bold text-3xl text-center select-none">Wallet</h3>

        <div>
            @if (session()->has('message'))
                <div class="alert alert-success">{{ session('message') }}</div>
            @endif

            @if (session()->has('error'))
                <div class="alert alert-danger">{{ session('error') }}</div>
            @endif

            <x-pane title="Bitcoin balance">
                <h4 class="text-center">{{ $balance_btc }} sats</h4>
            </x-pane>

            <div class="my-16">
                <x-pane title="Withdraw bitcoin">
                    <!-- Payment Request Form -->
                    <form class="p-4" wire:submit.prevent="submitPaymentRequest">
                        <div>
                            <label for="payment_request">Bolt11 invoice:</label>
                            <x-chat.textarea type="text" id="payment_request" wire:model="payment_request"
                                             required></x-chat.textarea>
                            @error('payment_request') <span class="error">{{ $message }}</span> @enderror
                        </div>

                        <x-secondary-button class="mt-4" type="submit">Withdraw to invoice</x-secondary-button>
                    </form>
                </x-pane>
            </div>

            <livewire:agent-balance-summary/>
        </div>

    </div>
</div>