import type { MetaFunction } from '@remix-run/node';
import Navigation from '~/components/Navigation';

export const meta: MetaFunction = () => {
  return [
    { title: 'OpenAgents - Mobile App' },
    { name: 'description', content: 'OpenAgents Mobile App - Onyx' },
  ];
};

export default function MobileApp() {
  return (
    <>
      <Navigation />
      <div id="content">
        <h1>Mobile App - Onyx</h1>
        {/* Add your mobile app content here */}
      </div>
    </>
  );
}