import { Head } from "@inertiajs/react"
import { Buttons, Inputs } from "@/Components/ComponentLibrary"

export default function ComponentLibrary() {
  return (
    <>
      <Head title="Component Library" />
      <div className='font-mono bg-black text-white min-h-screen w-screen p-8'>
        <h1 className="text-3xl font-bold mb-8">Component Library</h1>
        <div className="space-y-16">
          <Buttons />
          <Inputs />
        </div>
      </div>
    </>
  );
}