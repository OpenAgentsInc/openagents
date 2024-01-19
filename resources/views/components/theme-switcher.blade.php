<button id="theme-switcher"
    class="relative inline-flex h-[24px] w-[34px] shrink-0 cursor-pointer border-2 border-transparent focus:outline-none focus-visible:ring-2 focus-visible:ring-white focus-visible:ring-opacity-75">
    <div
        class="bg-gray-200/60 dark:bg-white/10 rounded-full absolute left-0 right-0 h-[0.65rem] top-1/2 translate-y-[-50%]">
    </div><span class="sr-only">Switch theme</span><span aria-hidden="true"
        class="translate-x-0 dark:translate-x-2.5 shadow shadow-gray-700/10 border border-gray-200 bg-background-light dark:border-primary-light dark:bg-background-dark p-[3px] pointer-events-none inline-block h-5 w-5 transform rounded-full ring-0 transition-transform duration-200 ease-in-out"><svg
            xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" class="dark:hidden">
            <path class="fill-yellow-500"
                d="M3.828 5.243L2.343 3.757a1 1 0 011.414-1.414l1.486 1.485a5.027 5.027 0 00-1.415 1.415zM7 3.1V1a1 1 0 112 0v2.1a5.023 5.023 0 00-2 0zm3.757.728l1.486-1.485a1 1 0 111.414 1.414l-1.485 1.486a5.027 5.027 0 00-1.415-1.415zM12.9 7H15a1 1 0 010 2h-2.1a5.023 5.023 0 000-2zm-.728 3.757l1.485 1.486a1 1 0 11-1.414 1.414l-1.486-1.485a5.027 5.027 0 001.415-1.415zM9 12.9V15a1 1 0 01-2 0v-2.1a5.023 5.023 0 002 0zm-3.757-.728l-1.486 1.485a1 1 0 01-1.414-1.414l1.485-1.486a5.027 5.027 0 001.415 1.415zM3.1 9H1a1 1 0 110-2h2.1a5.023 5.023 0 000 2zM8 11a3 3 0 110-6 3 3 0 010 6z"
                fill-rule="evenodd"></path>
        </svg><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" class="hidden dark:block">
            <path class="fill-primary-light"
                d="M7.914 0a6.874 6.874 0 00-1.26 3.972c0 3.875 3.213 7.017 7.178 7.017.943 0 1.843-.178 2.668-.5C15.423 13.688 12.34 16 8.704 16 4.174 16 .5 12.41.5 7.982.5 3.814 3.754.389 7.914 0z"
                fill-rule="evenodd"></path>
        </svg></span>
</button>

<script>
    // Function to toggle the dark theme
    function toggleDarkTheme() {
        // Check if the dark class is already on the HTML element
        const isDarkMode = document.documentElement.classList.contains('dark');

        // Toggle the dark class
        document.documentElement.classList.toggle('dark', !isDarkMode);

        // Update localStorage with the new theme state
        localStorage.setItem('theme', isDarkMode ? 'light' : 'dark');
    }

    // Initialize theme based on localStorage or system preference
    function initializeTheme() {
        // Check for saved theme in localStorage
        const savedTheme = localStorage.getItem('theme');
        const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

        // Apply the dark class based on saved theme or system preference
        if (savedTheme === 'dark' || (!savedTheme && prefersDark)) {
            document.documentElement.classList.add('dark');
        } else {
            document.documentElement.classList.remove('dark');
        }
    }

    // Add event listener to the theme switcher button
    document.getElementById('theme-switcher').addEventListener('click', toggleDarkTheme);

    // Initialize the theme when the script loads
    initializeTheme();

</script>
