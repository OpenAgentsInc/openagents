import { Button } from "@/Components/ui/button";
import { Head } from "@inertiajs/react";
import { GitHubLogoIcon, TwitterLogoIcon } from "@radix-ui/react-icons";

export default function Splash() {
    return <>
        <Head title="Home" />
        <div className="h-screen w-screen flex flex-col justify-center items-center">
            <p className="-mt-12 mb-12 text-7xl text-center tracking-widest">🌐🤖</p>
            <p className="italic mb-12 text-xl text-center tracking-widest text-gray-500">Soon</p>
            <div className="flex flex-row space-x-6">
                <a href="https://github.com/ArcadeLabsInc/openagents" target="_blank">
                    <Button variant="outline" size="icon" className="shadow-xl">
                        <GitHubLogoIcon className="w-6 h-6" />
                    </Button>
                </a>
                <a href="https://twitter.com/GPUtopia/status/1721942435125715086" target="_blank">
                    <Button variant="outline" size="icon" className="shadow-xl">
                        <TwitterLogoIcon className="w-6 h-6" />
                    </Button>
                </a>
            </div>

        </div>
    </>
}
