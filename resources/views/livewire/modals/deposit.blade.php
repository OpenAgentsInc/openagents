


    <form class="p-4" wire:submit.prevent="submitDepositRequest"
    class="p-4 w-full flex-col items-center justify-center text-center">
        <div >
            <h4>Lightning Deposit</h4>
               <p class="text-gray mt-4 text-xs">
                You can deposit any amount between {{ $min_deposit_sats }} and {{ $max_deposit_sats }} sats.
                </p>
            <label for="amount">Amount</label>
<x-input type="number" class="w-full"
wire:model="amount"
step="1" min="{{ $min_deposit_sats }}" max="{{ $max_deposit_sats }}" value="1000" />
            @error('amount') <span class="error">{{ $message }}</span> @enderror
        </div>

        <x-secondary-button class="mt-4 w-full" type="submit">Create invoice</x-secondary-button>
    </form>

