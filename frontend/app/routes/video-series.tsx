import type { Route, VideoEntry } from "../+types/video-series";
import { VIDEOS } from "../+types/video-series"

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Video Series" },
    { name: "description", content: "OpenAgents Video Series" },
  ];
}

function VideoEntry({ title, date, description, tweetUrl }: VideoEntry) {
  return (
    <div className="border-b pb-4">
      <div className="flex items-center gap-4 mb-2">
        <h2 className="text-base font-semibold">{title}</h2>
        <p className="text-white/50 text-sm">{date}</p>
      </div>
      <p className="text-sm mb-4">{description}</p>
      <a
        href={tweetUrl}
        target="_blank"
        className="bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center gap-2 whitespace-nowrap select-none text-center align-middle no-underline outline-none px-4 py-1 border border-white shadow-nav hover:shadow-nav-hover active:shadow-nav-active transition-all duration-nav ease-nav mt-2 mb-2"
      >
        Watch on X
      </a>
    </div>
  );
}

export default function VideoSeries() {
  return (
    <div className="container mx-auto px-4 max-w-4xl">
      <h1 className="text-lg font-bold mb-6">Recent Videos</h1>

      <p className="text-sm mb-6">
        We've documented a year of development in 150+ videos on X. Check out{" "}
        <a
          href="https://twitter.com/OpenAgentsInc/status/1721942435125715086"
          target="_blank"
          className="text-white/50 hover:text-white/80 underline"
        >
          episode one
        </a>{" "}
        or see the{" "}
        <a
          href="https://github.com/OpenAgentsInc/openagents/wiki/Video-Series"
          target="_blank"
          className="text-white/50 hover:text-white/80 underline"
        >
          full episode list
        </a>
        .
      </p>

      <div className="space-y-6">
        {VIDEOS.map((video, index) => (
          <VideoEntry key={index} {...video} />
        ))}
      </div>
    </div>
  );
}
