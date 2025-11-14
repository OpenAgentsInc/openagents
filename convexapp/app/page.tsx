"use client";

import { useConvexAuth, useMutation, useQuery } from "convex/react";
import { api } from "../convex/_generated/api";
import Link from "next/link";
import { useAuthActions } from "@convex-dev/auth/react";
import { useRouter } from "next/navigation";
import Image from "next/image";

export default function Home() {
  return (
    <>
      <header className="sticky top-0 z-10 bg-background/80 backdrop-blur-md p-4 border-b border-slate-200 dark:border-slate-700 flex flex-row justify-between items-center shadow-sm">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-3">
            <Image src="/convex.svg" alt="Convex Logo" width={32} height={32} />
            <div className="w-px h-8 bg-slate-300 dark:bg-slate-600"></div>
            <Image
              src="/nextjs-icon-light-background.svg"
              alt="Next.js Logo"
              width={32}
              height={32}
              className="dark:hidden"
            />
            <Image
              src="/nextjs-icon-dark-background.svg"
              alt="Next.js Logo"
              width={32}
              height={32}
              className="hidden dark:block"
            />
          </div>
          <h1 className="font-semibold text-slate-800 dark:text-slate-200">
            Convex + Next.js + Convex Auth
          </h1>
        </div>
        <SignOutButton />
      </header>
      <main className="p-8 flex flex-col gap-8">
        <Content />
      </main>
    </>
  );
}

function SignOutButton() {
  const { isAuthenticated } = useConvexAuth();
  const { signOut } = useAuthActions();
  const router = useRouter();
  return (
    <>
      {isAuthenticated && (
        <button
          className="bg-slate-600 hover:bg-slate-700 dark:bg-slate-700 dark:hover:bg-slate-600 text-white rounded-lg px-4 py-2 text-sm font-medium transition-all duration-200 shadow-sm hover:shadow-md cursor-pointer"
          onClick={() =>
            void signOut().then(() => {
              router.push("/signin");
            })
          }
        >
          Sign out
        </button>
      )}
    </>
  );
}

function Content() {
  const { viewer, numbers } =
    useQuery(api.myFunctions.listNumbers, {
      count: 10,
    }) ?? {};
  const addNumber = useMutation(api.myFunctions.addNumber);

  if (viewer === undefined || numbers === undefined) {
    return (
      <div className="mx-auto">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 bg-slate-400 rounded-full animate-bounce"></div>
          <div
            className="w-2 h-2 bg-slate-500 rounded-full animate-bounce"
            style={{ animationDelay: "0.1s" }}
          ></div>
          <div
            className="w-2 h-2 bg-slate-600 rounded-full animate-bounce"
            style={{ animationDelay: "0.2s" }}
          ></div>
          <p className="ml-2 text-slate-600 dark:text-slate-400">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 max-w-lg mx-auto">
      <div>
        <h2 className="font-bold text-xl text-slate-800 dark:text-slate-200">
          Welcome {viewer ?? "Anonymous"}!
        </h2>
        <p className="text-slate-600 dark:text-slate-400 mt-2">
          You are signed into a demo application using Convex Auth.
        </p>
        <p className="text-slate-600 dark:text-slate-400 mt-1">
          This app can generate random numbers and store them in your Convex
          database.
        </p>
      </div>

      <div className="h-px bg-slate-200 dark:bg-slate-700"></div>

      <div className="flex flex-col gap-4">
        <h2 className="font-semibold text-xl text-slate-800 dark:text-slate-200">
          Number generator
        </h2>
        <p className="text-slate-600 dark:text-slate-400 text-sm">
          Click the button below to generate a new number. The data is persisted
          in the Convex cloud database - open this page in another window and
          see the data sync automatically!
        </p>
        <button
          className="bg-slate-700 hover:bg-slate-800 dark:bg-slate-600 dark:hover:bg-slate-500 text-white text-sm font-medium px-6 py-3 rounded-lg cursor-pointer transition-all duration-200 shadow-md hover:shadow-lg hover:scale-[1.02] active:scale-[0.98]"
          onClick={() => {
            void addNumber({ value: Math.floor(Math.random() * 10) });
          }}
        >
          + Generate random number
        </button>
        <div className="bg-slate-100 dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded-xl p-4 shadow-sm">
          <p className="font-semibold text-slate-800 dark:text-slate-200 mb-2">
            Newest Numbers
          </p>
          <p className="text-slate-700 dark:text-slate-300 font-mono text-lg">
            {numbers?.length === 0
              ? "Click the button to generate a number!"
              : (numbers?.join(", ") ?? "...")}
          </p>
        </div>
      </div>

      <div className="h-px bg-slate-200 dark:bg-slate-700"></div>

      <div className="flex flex-col gap-4">
        <h2 className="font-semibold text-xl text-slate-800 dark:text-slate-200">
          Making changes
        </h2>
        <p className="text-slate-600 dark:text-slate-400 text-sm">
          Edit{" "}
          <code className="text-sm font-semibold font-mono bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-300 px-2 py-1 rounded-md border border-slate-300 dark:border-slate-600">
            convex/myFunctions.ts
          </code>{" "}
          to change the backend.
        </p>
        <p className="text-slate-600 dark:text-slate-400 text-sm">
          Edit{" "}
          <code className="text-sm font-semibold font-mono bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-300 px-2 py-1 rounded-md border border-slate-300 dark:border-slate-600">
            app/page.tsx
          </code>{" "}
          to change the frontend.
        </p>
        <p className="text-slate-600 dark:text-slate-400 text-sm">
          See the{" "}
          <Link
            href="/server"
            className="text-slate-700 dark:text-slate-300 hover:text-slate-900 dark:hover:text-slate-100 font-medium underline decoration-2 underline-offset-2 transition-colors"
          >
            /server route
          </Link>{" "}
          for an example of loading data in a server component
        </p>
      </div>

      <div className="h-px bg-slate-200 dark:bg-slate-700"></div>

      <div className="flex flex-col gap-4">
        <h2 className="text-xl font-bold text-slate-800 dark:text-slate-200">
          Useful resources
        </h2>
        <div className="flex gap-4">
          <div className="flex flex-col gap-4 w-1/2">
            <ResourceCard
              title="Convex docs"
              description="Read comprehensive documentation for all Convex features."
              href="https://docs.convex.dev/home"
            />
            <ResourceCard
              title="Stack articles"
              description="Learn about best practices, use cases, and more from a growing
            collection of articles, videos, and walkthroughs."
              href="https://stack.convex.dev"
            />
          </div>
          <div className="flex flex-col gap-4 w-1/2">
            <ResourceCard
              title="Templates"
              description="Browse our collection of templates to get started quickly."
              href="https://www.convex.dev/templates"
            />
            <ResourceCard
              title="Discord"
              description="Join our developer community to ask questions, trade tips & tricks,
            and show off your projects."
              href="https://www.convex.dev/community"
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function ResourceCard({
  title,
  description,
  href,
}: {
  title: string;
  description: string;
  href: string;
}) {
  return (
    <a
      href={href}
      className="flex flex-col gap-2 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 p-5 rounded-xl h-36 overflow-auto border border-slate-300 dark:border-slate-600 hover:border-slate-400 dark:hover:border-slate-500 shadow-sm hover:shadow-md transition-all duration-200 hover:scale-[1.02] group cursor-pointer"
      target="_blank"
    >
      <h3 className="text-sm font-semibold text-slate-700 dark:text-slate-300 group-hover:text-slate-900 dark:group-hover:text-slate-100 transition-colors">
        {title} →
      </h3>
      <p className="text-xs text-slate-600 dark:text-slate-400">
        {description}
      </p>
    </a>
  );
}
