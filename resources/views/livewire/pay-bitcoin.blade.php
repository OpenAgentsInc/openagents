<div>
    <livewire:navbar/>

    <div class="pt-32 text-center flex flex-col justify-center items-center">
        <h1>Add agent credit</h1>
        <h4 class="mt-4 text-gray">Pay with Bitcoin Lightning</h4>

        @if (!$qr)
            <form wire:submit="generateInvoice">
                <div class="mt-16 w-[350px] flex flex-col text-left">
                    <x-label for="amount">Amount (sats)</x-label>
                    <div class="w-full">
                        <input type="hidden" name="amount" id="slider-input"/>
                        <x-slider min="10" max="5000" step="10" value="{{ $amount }}"/>
                    </div>
                </div>

                <x-button size="lg" class="mt-4">Generate invoice</x-button>
            </form>
        @else
            <img class="mt-12 w-[300px]" src="{{ $qr }}" alt="QR code">
        @endif
    </div>
</div>