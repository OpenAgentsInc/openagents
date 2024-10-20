document.addEventListener('DOMContentLoaded', function() {
    const sidebar = document.getElementById('sidebar');
    const sidebarToggle = document.getElementById('sidebarToggle');
    const sidebarContent = document.getElementById('sidebarContent');
    const sidebarHeader = document.getElementById('sidebarHeader');
    const newTeamButton = document.getElementById('newTeamButton');
    const sidebarDivider = document.getElementById('sidebarDivider');

    // Get the initial state from localStorage
    let sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';

    function updateSidebarState(immediate = false) {
        const newWidth = sidebarCollapsed ? '70px' : '270px';
        const newOpacity = sidebarCollapsed ? '0' : '1';
        const newVisibility = sidebarCollapsed ? 'hidden' : 'visible';

        document.documentElement.style.setProperty('--sidebar-width', newWidth);
        document.documentElement.style.setProperty('--sidebar-content-opacity', newOpacity);
        document.documentElement.style.setProperty('--sidebar-content-visibility', newVisibility);

        sidebar.style.width = newWidth;
        [sidebarContent, sidebarHeader, newTeamButton, sidebarDivider].forEach(el => {
            if (el) {
                el.style.opacity = newOpacity;
                el.style.visibility = newVisibility;
            }
        });

        if (immediate) {
            document.documentElement.classList.add('sidebar-init');
        } else {
            requestAnimationFrame(() => {
                document.documentElement.classList.remove('sidebar-init');
            });
        }

        // Dispatch custom event
        document.dispatchEvent(new CustomEvent('sidebar-toggled', {
            detail: !sidebarCollapsed
        }));
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

    function toggleSection(sectionId) {
        const content = document.getElementById(sectionId);
        const isHidden = content.classList.contains('hidden');
        content.classList.toggle('hidden', !isHidden);
        const button = content.previousElementSibling;
        const svg = button.querySelector('svg:last-child');
        svg.style.transform = isHidden ? 'rotate(180deg)' : '';
    }

    window.toggleSection = toggleSection;
});