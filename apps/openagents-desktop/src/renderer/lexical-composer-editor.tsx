import { LexicalComposer, type InitialConfigType } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { OnChangePlugin } from "@lexical/react/LexicalOnChangePlugin";
import { PlainTextPlugin } from "@lexical/react/LexicalPlainTextPlugin";
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createTextNode,
  $getRoot,
  COMMAND_PRIORITY_HIGH,
  KEY_ENTER_COMMAND,
  type EditorState,
  type LexicalEditor,
} from "lexical";
import {
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  type ReactElement,
  type RefObject,
} from "react";

const EXTERNAL_VALUE_TAG = "openagents-composer-external-value";

/**
 * Lexical remains a renderer-private editing engine. Effect Native continues
 * to own the prompt value and every action intent. Register future file,
 * skill, and terminal DecoratorNodes here without changing that boundary.
 */
export const openAgentsComposerNodes: NonNullable<InitialConfigType["nodes"]> = [];

export interface LexicalComposerEditorHandle {
  readonly focus: () => void;
  readonly focusAtEnd: () => void;
  readonly readValue: () => string;
}

const setPlainText = (value: string): void => {
  const root = $getRoot();
  root.clear();
  const paragraph = $createParagraphNode();
  const lines = value.split("\n");
  lines.forEach((line, index) => {
    if (line !== "") paragraph.append($createTextNode(line));
    if (index < lines.length - 1) paragraph.append($createLineBreakNode());
  });
  root.append(paragraph);
  paragraph.selectEnd();
};

const readPlainText = (editorState: EditorState): string => {
  let value = "";
  editorState.read(() => {
    value = $getRoot().getTextContent();
  });
  return value;
};

const ControlledValuePlugin = ({ value }: { readonly value: string }): null => {
  const [editor] = useLexicalComposerContext();
  useLayoutEffect(() => {
    const current = readPlainText(editor.getEditorState());
    if (current === value) return;
    editor.update(() => setPlainText(value), { tag: EXTERNAL_VALUE_TAG });
  }, [editor, value]);
  return null;
};

const EditorHandlePlugin = ({
  editorRef,
}: {
  readonly editorRef: RefObject<LexicalComposerEditorHandle | null>;
}): null => {
  const [editor] = useLexicalComposerContext();
  useImperativeHandle(editorRef, () => ({
    focus: () => editor.focus(),
    focusAtEnd: () => {
      editor.update(() => $getRoot().selectEnd());
      editor.focus(undefined, { defaultSelection: "rootEnd" });
    },
    readValue: () => readPlainText(editor.getEditorState()),
  }), [editor]);
  return null;
};

const SubmitPlugin = ({
  onSubmit,
}: {
  readonly onSubmit: (value: string) => void;
}): null => {
  const [editor] = useLexicalComposerContext();
  const onSubmitRef = useRef(onSubmit);
  useLayoutEffect(() => {
    onSubmitRef.current = onSubmit;
  }, [onSubmit]);
  useEffect(
    () => editor.registerCommand(
      KEY_ENTER_COMMAND,
      (event: KeyboardEvent | null) => {
        if (event === null || event.shiftKey) return false;
        if (event.isComposing || event.keyCode === 229) return true;
        event.preventDefault();
        event.stopPropagation();
        onSubmitRef.current(readPlainText(editor.getEditorState()));
        return true;
      },
      COMMAND_PRIORITY_HIGH,
    ),
    [editor],
  );
  return null;
};

/**
 * Electron smoke probes historically address form controls through `.value`.
 * Keep that bounded automation seam while the real editor remains a Lexical
 * contenteditable; product code never reads this property.
 */
const AutomationValueBridgePlugin = (): null => {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    const root = editor.getRootElement() as (HTMLElement & { value?: string }) | null;
    if (root === null) return;
    Object.defineProperty(root, "value", {
      configurable: true,
      get: () => readPlainText(editor.getEditorState()),
      set: (next: unknown) => editor.update(
        () => setPlainText(typeof next === "string" ? next : String(next ?? "")),
      ),
    });
    return () => {
      Reflect.deleteProperty(root, "value");
    };
  }, [editor]);
  return null;
};

export interface LexicalComposerEditorProps {
  readonly value: string;
  readonly placeholder: string;
  readonly ariaLabel: string;
  readonly disabled: boolean;
  readonly editorRef: RefObject<LexicalComposerEditorHandle | null>;
  readonly onChange: (value: string) => void;
  readonly onSubmit: (value: string) => void;
}

export const LexicalComposerEditor = ({
  value,
  placeholder,
  ariaLabel,
  disabled,
  editorRef,
  onChange,
  onSubmit,
}: LexicalComposerEditorProps): ReactElement => {
  const onChangeRef = useRef(onChange);
  useLayoutEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);
  const handleChange = useCallback((editorState: EditorState, _editor: LexicalEditor, tags: Set<string>) => {
    if (tags.has(EXTERNAL_VALUE_TAG)) return;
    onChangeRef.current(readPlainText(editorState));
  }, []);
  const initialConfig = useMemo<InitialConfigType>(() => ({
    namespace: "OpenAgentsCodexComposer",
    editable: !disabled,
    nodes: [...openAgentsComposerNodes],
    editorState: () => setPlainText(value),
    onError(error: Error) {
      throw error;
    },
    theme: {
      paragraph: "oa-lexical-composer-paragraph",
    },
  }), []);

  return (
    <LexicalComposer initialConfig={initialConfig}>
      <div className="oa-lexical-composer-editor" data-editor-disabled={disabled ? "true" : "false"}>
        <PlainTextPlugin
          contentEditable={
            <ContentEditable
              className="oa-lexical-composer-content"
              aria-label={ariaLabel}
              aria-multiline="true"
              aria-placeholder={placeholder}
              placeholder={<div className="oa-lexical-composer-placeholder">{placeholder}</div>}
              spellCheck
              data-lexical-composer="true"
            />
          }
          ErrorBoundary={LexicalErrorBoundary}
        />
        <HistoryPlugin />
        <OnChangePlugin
          ignoreSelectionChange
          onChange={handleChange}
        />
        <ControlledValuePlugin value={value} />
        <EditorHandlePlugin editorRef={editorRef} />
        <SubmitPlugin onSubmit={onSubmit} />
        <EditableStatePlugin editable={!disabled} />
        <AutomationValueBridgePlugin />
      </div>
    </LexicalComposer>
  );
};

const EditableStatePlugin = ({ editable }: { readonly editable: boolean }): null => {
  const [editor] = useLexicalComposerContext();
  useEffect(() => editor.setEditable(editable), [editor, editable]);
  return null;
};
