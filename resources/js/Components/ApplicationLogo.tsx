import { Link } from "@inertiajs/react";

export default function ApplicationLogo() {
  const strokeColor = "black"
  const strokeWidth = 14
  return (
    // <Link href="/">
    <div className="flex flex-row items-center" >
      <svg viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg" className="h-9 w-auto">
        <circle cx="100" cy="100" r="90" stroke={strokeColor} strokeWidth={strokeWidth} fill="none" />
        <path d="M 30 150 L 100 78 L 170 150" stroke={strokeColor} strokeWidth={strokeWidth} fill="none" />
      </svg>
      <h1 className="ml-1 text-2xl font-light tracking-tight">Open<span className="font-bold">Agents</span></h1>
    </div>
    // </Link>
  );
}
