import type { MetaFunction } from 'react-router';
import Navigation from '~/components/Navigation';

export const meta: MetaFunction = () => {
  return [
    { title: 'OpenAgents - Services' },
    { name: 'description', content: 'OpenAgents Business Services' },
  ];
};

export default function Services() {
  return (
    <>
      <Navigation />
      <div id="content">
        <h1>Services</h1>
        {/* Add your services content here */}
      </div>
    </>
  );
}