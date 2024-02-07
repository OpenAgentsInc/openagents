<button
    {{ $attributes->merge(['type' => 'submit', 'class' => 'inline-flex items-center px-4 py-2 bg-gray border border-transparent rounded-md font-semibold text-xs text-black dark:text-white uppercase tracking-widest hover:bg-white dark:hover:bg-gray focus:bg-white dark:focus:bg-gray active:bg-gray focus:outline-none focus:ring-2 focus:ring-black focus:ring-offset-2 dark:focus:ring-offset-black transition ease-in-out duration-150']) }}>
    {{ $slot }}
</button>
