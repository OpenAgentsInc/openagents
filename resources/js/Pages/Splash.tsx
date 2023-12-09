import { Button } from "@/Components/ui/button";
import SimpleLayout from "@/Layouts/SimpleLayout";
import { GitHubLogoIcon } from "@radix-ui/react-icons";

function Splash() {
    return <div className="h-[90vh] w-screen flex flex-col justify-center items-center">
        <p className="-mt-12 mb-12 text-7xl text-center tracking-widest">🌐🤖</p>
        <p className="font-mono mb-12 text-3xl text-center tracking-widest text-gray-500">Soon</p>
        <div className="flex flex-row space-x-6">
            <a href="https://github.com/ArcadeLabsInc/openagents" target="_blank">
                <Button variant="outline" size="icon" className="shadow-xl">
                    <GitHubLogoIcon className="w-6 h-6" />
                </Button>
            </a>
            <a href="https://twitter.com/GPUtopia/status/1721942435125715086" target="_blank">
                <Button variant="outline" size="icon" className="shadow-xl">
                    <img src="/images/x-logo-black.png" className="w-6 h-6" />
                </Button>
            </a>
        </div>
    </div>
}

Splash.layout = (page) => <SimpleLayout children={page} title="Inspect" />

export default Splash
