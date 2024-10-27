import React from "react"
import { Head } from "@inertiajs/react"

interface Props {
  content: string;
  title: string;
}

export default function Show({ content, title }: Props) {
  return (
    <>
      <Head title={title} />
      <div className="min-h-screen py-12 bg-background">
        <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="bg-card shadow-sm rounded-lg">
            <article className="p-8 dark:prose-invert prose prose-sm sm:prose lg:prose-lg xl:prose-xl mx-auto">
              <div dangerouslySetInnerHTML={{ __html: content }} />
            </article>
          </div>
        </div>
      </div>
    </>
  );
}
