import { useState } from "react";
import { EditorState } from "prosemirror-state";
import { schema } from "prosemirror-schema-basic";
import "prosemirror-view/style/prosemirror.css";
import { ProseMirror } from "@nytimes/react-prosemirror";

const defaultState = EditorState.create({ schema });

export const ChatInput = () => {
  const [mount, setMount] = useState<HTMLElement | null>(null);
  return (
    <fieldset className="flex w-full min-w-0 flex-col-reverse">
      <div className="flex  flex-col  bg-zinc-900  gap-1.5  border-0.5  border-border-300  pl-4  pt-2.5  pr-2.5  pb-2.5  -mx-1  sm:mx-0  items-stretch  transition-all  duration-200  relative  shadow-[0_0.25rem_1.25rem_rgba(0,0,0,0.035)]  focus-within:shadow-[0_0.25rem_1.25rem_rgba(0,0,0,0.075)]  hover:border-border-200  focus-within:border-border-200  cursor-text  z-10 rounded-t-2xl border-b-0">
        <div className="flex gap-2">
          <div
            aria-label="Write your prompt"
            className="mt-1 max-h-96 w-full overflow-y-auto break-words outline-none focus:outline-none"
          >
            <ProseMirror mount={mount} defaultState={defaultState}>
              <div ref={setMount} />
            </ProseMirror>
          </div>
        </div>
      </div>
    </fieldset>
  );
};
