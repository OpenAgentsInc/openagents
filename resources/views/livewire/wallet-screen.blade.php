<div class="p-4 md:p-12 mx-auto flex flex-col justify-center w-full items-center" x-data="{ dropdown: false }">
    <div class="w-full md:max-w-3xl md:min-w-[600px]">
        <h3 class="mb-16 font-bold text-3xl text-center select-none">Wallet</h3>
        <p class="text-center text-lg">Balance: {{ $this->balance_btc / 1000 }} sats</p>
    </div>
</div>