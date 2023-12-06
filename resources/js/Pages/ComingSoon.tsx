import InspectLayout from "@/Layouts/InspectLayout";

function ComingSoon() {
    return (
        <div className="w-full px-12 mx-auto flex flex-col justify-center items-center">
            <main className="w-full">
                <div className="w-full relative">
                    <div className="py-24 sm:py-32">
                        <div className="mx-auto max-w-7xl px-6 lg:px-8">
                            <div className="mx-auto max-w-2xl text-center">
                                <h1 className="text-4xl font-bold tracking-tight text-gray-900 sm:text-6xl">
                                    Make AI work for you.
                                </h1>
                                <p className="mt-6 text-lg leading-8 text-gray-600">
                                    Train your own AI agent &mdash; no code required. <span className="font-bold">Coming soon.</span>
                                </p>
                            </div>
                            <div className="mt-16 flow-root sm:mt-24">
                                <div className="-m-2 rounded-xl bg-gray-900/5 p-2 ring-1 ring-inset ring-gray-900/10 lg:-m-4 lg:rounded-2xl lg:p-4">
                                    <img
                                        src="https://tailwindui.com/img/component-images/project-app-screenshot.png"
                                        alt="App screenshot"
                                        width={2432}
                                        height={1442}
                                        className="rounded-md shadow-2xl ring-1 ring-gray-900/10"
                                    />
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </main>

        </div>
    )
}

ComingSoon.layout = (page) => <InspectLayout children={page} title="Home" />

export default ComingSoon
