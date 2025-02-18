import type { MetaFunction } from 'react-router';
import Navigation from '~/components/Navigation';

export const meta: MetaFunction = () => {
  return [
    { title: 'OpenAgents - Company' },
    { name: 'description', content: 'About OpenAgents' },
  ];
};

export default function Company() {
  return (
    <>
      <Navigation />
      <div id="content">
        <h1>Company</h1>
        {/* Add your company content here */}
      </div>
    </>
  );
}