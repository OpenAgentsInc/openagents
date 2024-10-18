document.addEventListener('DOMContentLoaded', function() {
    const sidebar = document.getElementById('sidebar');
    const sidebarToggle = document.getElementById('sidebarToggle');
    const sidebarContent = document.getElementById('sidebarContent');

    // Get the initial state from localStorage
    let sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';

    function updateSidebarState(immediate = false) {
        const newWidth = sidebarCollapsed ? '70px' : '270px';
        document.documentElement.style.setProperty('--sidebar-width', newWidth);

        if (sidebarCollapsed) {
            sidebarContent.style.opacity = '0';
            sidebarContent.style.visibility = 'hidden';
        } else {
            sidebarContent.style.opacity = '1';
            sidebarContent.style.visibility = 'visible';
        }

        if (immediate) {
            document.documentElement.classList.add('sidebar-init');
        } else {
            // Re-enable transitions after the initial state is set
            setTimeout(() => {
                document.documentElement.classList.remove('sidebar-init');
            }, 0);
        }
    }

    // Set initial state
    updateSidebarState(true);

    sidebarToggle.addEventListener('click', function() {
        sidebarCollapsed = !sidebarCollapsed;
        localStorage.setItem('sidebarCollapsed', sidebarCollapsed);
        updateSidebarState();
    });
});