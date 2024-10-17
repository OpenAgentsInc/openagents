<style>
    .bg-grid {
        background-image:
            linear-gradient(to right, rgba(255, 255, 255, 0.2) 1px, transparent 1px),
            linear-gradient(to bottom, rgba(255, 255, 255, 0.2) 1px, transparent 1px);
        background-size: 40px 40px;
    }

    .neural-network {
        background:
            radial-gradient(circle at 50% 50%, rgba(255, 255, 255, 0.1) 0%, transparent 25%),
            radial-gradient(circle at 20% 80%, rgba(255, 255, 255, 0.1) 0%, transparent 35%),
            radial-gradient(circle at 80% 20%, rgba(255, 255, 255, 0.1) 0%, transparent 35%);
        animation: move 30s ease-in-out infinite alternate;
    }

    @keyframes move {
        0% {
            transform: translate(0, 0);
        }

        100% {
            transform: translate(40px, 40px);
        }
    }
</style>