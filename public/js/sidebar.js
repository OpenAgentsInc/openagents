document.addEventListener('DOMContentLoaded', function() {
    const sidebar = document.getElementById('sidebar');
    const sidebarContent = document.getElementById('sidebarContent');
    const sidebarHeader = document.getElementById('sidebarHeader');
    const newTeamButton = document.getElementById('newTeamButton');
    const sidebarDivider = document.getElementById('sidebarDivider');

    function toggleSidebar() {
        const isCollapsed = sidebar.style.width === '60px';

        sidebar.style.width = isCollapsed ? '270px' : '60px';
        [sidebarContent, sidebarHeader, newTeamButton, sidebarDivider].forEach(el => {
            el.style.opacity = isCollapsed ? '1' : '0';
        });

        // Dispatch custom event
        document.dispatchEvent(new CustomEvent('sidebar-toggled', {
            detail: !isCollapsed
        }));
    }

    document.getElementById('sidebarToggle').addEventListener('click', toggleSidebar);

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