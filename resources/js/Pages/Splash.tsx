import { Button } from "@/Components/ui/button";
import { Head } from "@inertiajs/react";
import { GitHubLogoIcon } from "@radix-ui/react-icons";

export default function Splash() {
    return <>
        <Head title="Home" />
        <div className="h-screen w-screen flex flex-col justify-center items-center">
            <p className="my-8 text-3xl text-center">Soon.</p>
            <a href="https://github.com/ArcadeLabsInc/openagents">
                <Button variant="outline" size="icon">
                    <GitHubLogoIcon className="w-6 h-6" />
                </Button>
            </a>
        </div>
    </>
}
