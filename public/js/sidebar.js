// Set initial sidebar state
(function() {
    var sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';
    document.documentElement.classList.add('sidebar-init');
    document.documentElement.style.setProperty('--sidebar-width', sidebarCollapsed ? '70px' : '270px');
    document.documentElement.style.setProperty('--sidebar-content-opacity', sidebarCollapsed ? '0' : '1');
    document.documentElement.style.setProperty('--sidebar-content-visibility', sidebarCollapsed ? 'hidden' : 'visible');
})();

document.addEventListener('DOMContentLoaded', function() {
    const sidebar = document.getElementById('sidebar');
    const sidebarToggle = document.getElementById('sidebarToggle');
    const sidebarContent = document.getElementById('sidebarContent');

    // Get the initial state from localStorage
    let sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';

    function updateSidebarState(immediate = false) {
        const newWidth = sidebarCollapsed ? '70px' : '270px';
        const newOpacity = sidebarCollapsed ? '0' : '1';
        const newVisibility = sidebarCollapsed ? 'hidden' : 'visible';

        document.documentElement.style.setProperty('--sidebar-width', newWidth);
        document.documentElement.style.setProperty('--sidebar-content-opacity', newOpacity);
        document.documentElement.style.setProperty('--sidebar-content-visibility', newVisibility);

        if (immediate) {
            document.documentElement.classList.add('sidebar-init');
        } else {
            requestAnimationFrame(() => {
                document.documentElement.classList.remove('sidebar-init');
            });
        }
    }

    // Set initial state
    updateSidebarState(true);

    // Remove the 'sidebar-init' class after a short delay to enable animations
    setTimeout(() => {
        document.documentElement.classList.remove('sidebar-init');
    }, 50);

    sidebarToggle.addEventListener('click', function() {
        sidebarCollapsed = !sidebarCollapsed;
        localStorage.setItem('sidebarCollapsed', sidebarCollapsed);
        updateSidebarState();
    });
});