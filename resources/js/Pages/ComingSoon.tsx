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
                    <div
                        className="absolute inset-x-0 top-[calc(100%-13rem)] -z-10 transform-gpu overflow-hidden blur-3xl sm:top-[calc(100%-30rem)]"
                        aria-hidden="true"
                    >
                        <div
                            className="relative left-[calc(50%+3rem)] aspect-[1155/678] w-[36.125rem] -translate-x-1/2 bg-gradient-to-tr from-[#ff80b5] to-[#9089fc] opacity-30 sm:left-[calc(50%+36rem)] sm:w-[72.1875rem]"
                            style={{
                                clipPath:
                                    'polygon(74.1% 44.1%, 100% 61.6%, 97.5% 26.9%, 85.5% 0.1%, 80.7% 2%, 72.5% 32.5%, 60.2% 62.4%, 52.4% 68.1%, 47.5% 58.3%, 45.2% 34.5%, 27.5% 76.7%, 0.1% 64.9%, 17.9% 100%, 27.6% 76.8%, 76.1% 97.7%, 74.1% 44.1%)',
                            }}
                        />
                    </div>
                </div>
            </main>

        </div>
    )
}

ComingSoon.layout = (page) => <InspectLayout children={page} title="Home" />

export default ComingSoon
