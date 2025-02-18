import type { MetaFunction } from 'react-router';
import Navigation from '~/components/Navigation';

export const meta: MetaFunction = () => {
  return [
    { title: 'OpenAgents - Home' },
    { name: 'description', content: 'Welcome to OpenAgents' },
  ];
};

export default function Home() {
  return (
    <>
      <Navigation />
      <div id="content">
        <h1>Welcome to OpenAgents</h1>
        {/* Add your home page content here */}
      </div>
    </>
  );
}