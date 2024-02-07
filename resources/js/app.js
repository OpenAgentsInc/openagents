// import Alpine from 'alpinejs';

// window.Alpine = Alpine;

// Alpine.start();

document.documentElement.classList.add('dark');

// Initialize theme based on localStorage or system preference
function initializeTheme() {
  // Check for saved theme in localStorage
  const savedTheme = localStorage.getItem('theme');
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

  // Apply the dark class based on saved theme or system preference
  if (savedTheme === 'dark' || (!savedTheme && prefersDark)) {
    // document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
}

// Initialize the theme when the script loads
document.addEventListener('DOMContentLoaded', initializeTheme);
