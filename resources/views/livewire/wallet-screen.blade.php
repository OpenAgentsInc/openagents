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
                <table class="w-full">
                    <tr>
                        <th  class="text-center">Available</th>
                        <th   class="text-center">Pending</th>

                    </tr>
                    <tr>
                        <td  class="text-center">{{ $balance_btc }} sats</td>
                        <th  class="text-center">{{ $pending_balance_btc }} sats</th>

                    </tr>

                </table>

                </p>

                <div class="px-4 mt-6 pt-2 flex justify-evenly">
                    <x-secondary-button  wire:click="withdraw">Withdraw</x-secondary-button>
                    <x-secondary-button  wire:click="deposit">Deposit</x-secondary-button>
                </div>

                <p class="text-gray mt-4 text-xs text-center">
                    Pending balances comprise payouts from agents, plugins and other revenue sources owned by you.
                    They become unlocked once per minute.
                    20% of the rewards are held by OpenAgents as platform fees.
                </p>

            </x-pane>




              <div class="my-16">
                <x-pane title="Your Lightning Address">
                    <div class="px-4 pt-2 ">
                        <p class=" text-gray">
                        You can receive payments between 1 and 10000 sats to this Lightning Address and they will be credited to your account.
                        </p>
                        <p class="text-xs text-gray
                        ">This feature is experimental. Do not send anything you aren't willing to lose!</p>
                         <p>
                            <livewire:lightning-address-display :lightning-address="$lightning_address" />
                        </p>

                    </div>
                    <div class="px-4 pt-2 ">
                        <h4>Custom address
                            <span class="inline-flex bg-opacity-15 bg-white rounded-md px-1 py-1 text-gray-500 text-sm flex justify-center items-center w-[56px] h-[20px]" >
                                <x-icon.logo class="w-[12px] h-[12px] mr-[4px]"/> Pro
                            </span>
                        </h4>
                        <p class="text-gray">
                            Pro users can choose a custom Lightning Address.
                        </p>
                        <p class="text-xs text-gray">
                            Valid characters: A-Z, a-z, 0-9, _, -, .
                        </p>
                        <div class="flex items-center gap-2
                        justify-start mt-2
                        ">

                        <x-input id="custom_lightning_address"
                         class="block mt-1 w-32 text-right"
                        placeholder="your_new_address"

                        pattern="[A-Za-z0-9_\-\.]*"
                        type="text" name="custom_lightning_address"
                        wire:model='custom_lightning_address'
                              required placeholder=""/>{{"@".$lightning_domain}}
                        <x-secondary-button class="h-6 ml-4"
                         wire:click="updateCustomLightningAddress">Save</x-secondary-button>

                        </div>
                        <div class="mt-4">
                            <h5>Address history</h5>
                            <p class="text-gray text-xs">You can use any address you've used before.</p>
                            <div class="flex flex-col gap-2">
                                @foreach($address_history as $address)
                                <livewire:lightning-address-display :lightning-address="$address" />


                                @endforeach
                            </div>
                        </div>


                    </div>


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
