document.addEventListener('DOMContentLoaded', function() {
    const sidebar = document.getElementById('sidebar');
    const sidebarToggle = document.getElementById('sidebarToggle');
    const sidebarContent = document.getElementById('sidebarContent');

    // Get the initial state from localStorage
    let sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';

    function updateSidebarState(immediate = false) {
        if (sidebarCollapsed) {
            sidebar.style.width = '70px';
            sidebarContent.style.opacity = '0';
            sidebarContent.style.visibility = 'hidden';
        } else {
            sidebar.style.width = '270px';
            sidebarContent.style.opacity = '1';
            sidebarContent.style.visibility = 'visible';
        }

        if (!immediate) {
            // Re-enable transitions after the initial state is set
            setTimeout(() => {
                document.body.classList.remove('sidebar-init');
            }, 0);
        }
    }

    // Set initial state immediately without transitions
    document.body.classList.add('sidebar-init');
    updateSidebarState(true);

    sidebarToggle.addEventListener('click', function() {
        sidebarCollapsed = !sidebarCollapsed;
        localStorage.setItem('sidebarCollapsed', sidebarCollapsed);
        updateSidebarState();
    });
});