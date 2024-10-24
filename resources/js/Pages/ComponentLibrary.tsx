import { Head } from "@inertiajs/react"
import { Buttons } from "@/Components/ComponentLibrary"

export default function ComponentLibrary() {
  return (
    <>
      <Head title="Component Library" />
      <div className='font-mono bg-black text-white min-h-screen w-screen p-8'>
        <h1 className="text-3xl font-bold mb-8">Component Library</h1>
        <Buttons />
      </div>
    </>
  );
}