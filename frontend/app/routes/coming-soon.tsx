import type { MetaFunction } from '@remix-run/node';
import Navigation from '~/components/Navigation';

export const meta: MetaFunction = () => {
  return [
    { title: 'OpenAgents - Coming Soon' },
    { name: 'description', content: 'Coming Soon - OpenAgents' },
  ];
};

export default function ComingSoon() {
  return (
    <>
      <Navigation />
      <div id="content">
        <h1>Coming Soon</h1>
        {/* Add your coming soon content here */}
      </div>
    </>
  );
}