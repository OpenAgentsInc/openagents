import type { CSSProperties } from "react";
const paths: Record<string, string> = {
  select: "m6 3 13 10-7 1-3 7-3-18Z",
  assets: "m12 3 9 5-9 5-9-5 9-5Zm-9 5v9l9 5 9-5V8M12 13v9",
  build: "m3 11 9-8 9 8M5 10v11h14V10M9 21v-8h6v8",
  command: "m4 6 5 6-5 6M12 18h8",
  settings: "M4 7h16M4 17h16M8 4v6M16 14v6",
  undo: "m8 4-5 5 5 5M3 9h10a7 7 0 0 1 0 14",
  redo: "m16 4 5 5-5 5M21 9H11a7 7 0 0 0 0 14",
  walk: "M13 5a2 2 0 1 0 0-4 2 2 0 0 0 0 4ZM8 21l3-7-1-5 4-2 2 5 4 1M14 14l3 7M5 12l4-3",
  fly: "m3 12 18-9-6 18-4-7-8-2Zm8 2 10-11",
  sun: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8ZM12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5",
  moon: "M20 15.5A9 9 0 0 1 8.5 4 9 9 0 1 0 20 15.5Z",
  camera: "M3 7h5l2-3h4l2 3h5v14H3V7Zm9 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z",
  close: "m6 6 12 12M6 18 18 6",
  chevron: "m7 10 5 5 5-5",
  trash: "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7",
  plus: "M12 4v16M4 12h16",
  check: "m4 12 5 5L20 6",
  arrow: "M4 12h16m-6-6 6 6-6 6",
  search: "M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14Zm5 12 6 6",
};
export function Icon({
  name,
  size = 20,
  style,
}: {
  name: string;
  size?: number;
  style?: CSSProperties;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={style}
    >
      <path d={paths[name] ?? paths.assets} />
    </svg>
  );
}
