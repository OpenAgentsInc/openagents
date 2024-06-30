import React, { useCallback, useState, useRef, useEffect } from "react";
import { EditorState } from "prosemirror-state";
import { Schema } from "prosemirror-model";
import { keymap } from "prosemirror-keymap";
import {
  baseKeymap,
  chainCommands,
  newlineInCode,
  createParagraphNear,
  liftEmptyBlock,
  splitBlock,
} from "prosemirror-commands";
import "prosemirror-view/style/prosemirror.css";
import { ProseMirror, useEditorEffect } from "@nytimes/react-prosemirror";
import { useForm } from "@inertiajs/react";
import { useMessageStore } from "../store";
import { useSSE } from "../hooks/useSSE";

const schema = new Schema({
  nodes: {
    doc: {
      content: "block+",
    },
    paragraph: {
      group: "block",
      content: "inline*",
      toDOM() {
        return ["p", 0];
      },
    },
    text: {
      group: "inline",
    },
  },
});

const createDefaultState = () => {
  return EditorState.create({
    schema,
    plugins: [
      keymap({
        ...baseKeymap,
        Enter: (state, dispatch) => {
          if (state.selection.empty && state.doc.content.size > 0) {
            if (dispatch) {
              dispatch(state.tr.setMeta("isEnter", true));
            }
            return true;
          }
          return false;
        },
        "Shift-Enter": chainCommands(
          newlineInCode,
          createParagraphNear,
          liftEmptyBlock,
          splitBlock
        ),
      }),
    ],
  });
};

export const ChatInput = () => {
  const [mount, setMount] = useState<HTMLElement | null>(null);
  const [editorState, setEditorState] = useState(createDefaultState);
  const [shouldReset, setShouldReset] = useState(false);
  const addMessage = useMessageStore((state) => state.addMessage);
  const messages = useMessageStore((state) => state.messages);

  const { data, setData, reset } = useForm({
    content: "",
  });

  const { startSSEConnection } = useSSE("/api/sse-stream");

  const handleEditorStateChange = useCallback(
    (state: EditorState) => {
      setEditorState(state);
      setData("content", state.doc.textContent);
    },
    [setData]
  );

  const handleSubmit = useCallback(() => {
    if (data.content.trim()) {
      addMessage(data.content, true, true); // Add user message to the store

      // Prepare the message history
      const messageHistory = messages
        .concat({ content: data.content, isUser: true })
        .map((msg) => ({
          role: msg.isUser ? "user" : "assistant",
          content: msg.content,
        }));

      startSSEConnection(messageHistory); // Start SSE connection with full message history

      setEditorState(createDefaultState());
      reset("content");
    }
  }, [data.content, addMessage, messages, startSSEConnection, reset]);

  useEffect(() => {
    if (shouldReset) {
      setEditorState(createDefaultState());
      setShouldReset(false);
    }
  }, [shouldReset]);

  return (
    <form onSubmit={(e) => e.preventDefault()}>
      <fieldset className="flex w-full min-w-0 flex-col-reverse">
        <div className="flex flex-col bg-zinc-900 gap-1.5 border-0.5 border-border-300 pl-4 pt-2.5 pr-2.5 pb-2.5 -mx-1 sm:mx-0 items-stretch transition-all duration-200 relative shadow-[0_0.25rem_1.25rem_rgba(0,0,0,0.035)] focus-within:shadow-[0_0.25rem_1.25rem_rgba(0,0,0,0.075)] hover:border-border-200 focus-within:border-border-200 cursor-text z-10 rounded-t-2xl border-b-0">
          <div className="flex gap-2">
            <div
              aria-label="Write your prompt"
              className="mt-1 max-h-96 w-full overflow-y-auto break-words outline-none focus:outline-none"
            >
              <ProseMirror
                mount={mount}
                state={editorState}
                dispatchTransaction={(tr) => {
                  const newState = editorState.apply(tr);
                  handleEditorStateChange(newState);

                  if (tr.getMeta("isEnter")) {
                    handleSubmit();
                  }
                }}
              >
                <div ref={setMount} />
                <EditorFocuser shouldReset={shouldReset} />
              </ProseMirror>
            </div>
          </div>
        </div>
      </fieldset>
    </form>
  );
};

const EditorFocuser = ({ shouldReset }: { shouldReset: boolean }) => {
  const focusRef = useRef<(() => void) | null>(null);

  useEditorEffect(
    (view) => {
      focusRef.current = () => view.focus();

      // Focus the editor after a short delay
      setTimeout(() => focusRef.current?.(), 0);

      return () => {
        focusRef.current = null;
      };
    },
    [shouldReset]
  ); // Re-run when shouldReset changes

  return null;
};
