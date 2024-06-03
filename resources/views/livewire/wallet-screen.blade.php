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

            <x-pane title="Your bitcoin balance">
                <h4 class="text-center">{{ $balance_btc }} sats</h4>
            </x-pane>

            <div class="my-16">
                <livewire:agent-balance-summary/>
            </div>

            <div class="my-16">
                <x-pane title="Withdraw bitcoin">
                    <div class="px-4 pt-2 text-gray">You can withdraw any amount up to {{ $balance_btc }} sats.</div>

                    <form class="p-4" wire:submit.prevent="submitPaymentRequest">
                        <div>
                            <label for="payment_request">Lightning invoice:</label>
                            <x-chat.textarea type="text" id="payment_request" wire:model="payment_request"
                                             required default="lnbc..."></x-chat.textarea>
                            @error('payment_request') <span class="error">{{ $message }}</span> @enderror
                        </div>

                        <x-secondary-button class="mt-4" type="submit">Withdraw to invoice</x-secondary-button>
                    </form>
                </x-pane>
            </div>

            <div class="my-16">
                <x-pane title="Recent payments received">
                    @foreach($received_payments as $payment)
                        <div class="p-4 border-b border-offblack">
                            <div class="flex justify-between">
                                <div>{{ $payment->amount / 1000 }} sats</div>
                                <div>{{ $payment->description }}</div>
                                <div>{{ $payment->created_at->diffForHumans() }}</div>
                            </div>
                        </div>
                    @endforeach
                </x-pane>
            </div>

            <div class="my-16">
                <x-pane title="Recent deposits">
                    @foreach($payins as $payin)
                        <div class="p-4 border-b border-offblack">
                            <div class="flex justify-between">
                                <div>{{ $payin->amount / 1000 }} sats</div>
                                <div>{{ $payin->description }}</div>
                                <div>{{ $payin->created_at->diffForHumans() }}</div>
                            </div>
                        </div>
                    @endforeach
                </x-pane>
            </div>
        </div>
    </div>
</div>