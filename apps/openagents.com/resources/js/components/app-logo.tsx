export default function AppLogo() {
    return (
        <div className="flex w-full items-center gap-2 group-data-[collapsible=icon]:justify-center">
            <img
                src="/favicon.ico"
                alt=""
                className="size-6 shrink-0 rounded-md object-contain"
            />
            <div className="logo-text ml-1 grid flex-1 min-w-0 text-left text-sm group-data-[collapsible=icon]:hidden">
                <span className="mb-0.5 truncate leading-tight font-semibold">
                    OpenAgents
                </span>
            </div>
        </div>
    );
}
