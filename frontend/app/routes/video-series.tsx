import type { Route } from "../+types/video-series";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Video Series" },
    { name: "description", content: "OpenAgents Video Series" },
  ];
}

export default function VideoSeries() {
  return (
    <div id="content">
      <h1>Video Series</h1>
      {/* Add your video series content here */}
    </div>
  );
}