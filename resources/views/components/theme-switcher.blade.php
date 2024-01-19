<button id="theme-switcher">
    dark or light
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
