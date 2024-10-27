import { Head } from "@inertiajs/react"

interface Props {
  content: string;
  title: string;
}

export default function Show({ content, title }: Props) {
  return (
    <>
      <Head title={title} />
      <div className="py-12">
        <div className="max-w-7xl mx-auto sm:px-6 lg:px-8">
          <div className="bg-background overflow-hidden shadow-sm sm:rounded-lg">
            <div className="p-6 prose dark:prose-invert prose-sm sm:prose lg:prose-lg xl:prose-xl max-w-none">
              <div dangerouslySetInnerHTML={{ __html: content }} />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
