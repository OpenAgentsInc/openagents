import type { MetaFunction } from 'react-router';
import Navigation from '~/components/Navigation';

export const meta: MetaFunction = () => {
  return [
    { title: 'OpenAgents - Video Series' },
    { name: 'description', content: 'OpenAgents Video Series' },
  ];
};

export default function VideoSeries() {
  return (
    <>
      <Navigation />
      <div id="content">
        <h1>Video Series</h1>
        {/* Add your video series content here */}
      </div>
    </>
  );
}