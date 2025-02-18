import {
  Links,
  Meta,
  Outlet,
  Scripts,
  ScrollRestoration,
} from 'react-router-dom';
import type { LinksFunction } from 'react-router';

import styles from './tailwind.css';

export const links: LinksFunction = () => [
  { rel: 'stylesheet', href: styles },
  { rel: 'stylesheet', href: '/assets/fonts.css' },
];

export default function App() {
  return (
    <html lang="en">
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
        <Meta />
        <Links />
      </head>
      <body className="min-h-screen w-screen bg-black text-white font-mono overflow-x-hidden">
        <div className="flex justify-center mx-2 md:mx-6">
          <div className="w-[60rem] max-w-full my-6 px-4 py-6 border border-white">
            <Outlet />
          </div>
        </div>
        <ScrollRestoration />
        <Scripts />
      </body>
    </html>
  );
}