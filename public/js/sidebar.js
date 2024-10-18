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
            if (immediate) {
                sidebarContent.style.visibility = 'hidden';
            } else {
                setTimeout(() => {
                    sidebarContent.style.visibility = 'hidden';
                }, 200); // Match this with the transition duration
            }
        } else {
            sidebar.style.width = '270px';
            sidebarContent.style.visibility = 'visible';
            setTimeout(() => {
                sidebarContent.style.opacity = '1';
            }, 50); // Small delay to ensure display change has taken effect
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