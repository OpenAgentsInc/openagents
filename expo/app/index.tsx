import { Redirect } from 'expo-router';

// Default entry should go straight to a new thread route
export default function Index() { return <Redirect href="/thread/new" /> }
