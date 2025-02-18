import type { MetaFunction } from '@remix-run/node';
import Navigation from '~/components/Navigation';

export const meta: MetaFunction = () => {
  return [
    { title: 'OpenAgents - Video Series' },
    { name: 'description', content: 'OpenAgents Video Series' },
  ];
};

export default function VideoSeries() {
  return (
    <div className="min-h-screen w-screen bg-black text-white font-mono overflow-x-hidden">
      <div className="flex justify-center mx-2 md:mx-6">
        <div className="w-[60rem] max-w-full my-6 px-4 py-6 border border-white">
          <Navigation />
          <div id="content">
            <h1>Video Series</h1>
            {/* Add your video series content here */}
          </div>
        </div>
      </div>
    </div>
  );
}