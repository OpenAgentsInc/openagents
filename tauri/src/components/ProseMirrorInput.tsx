'use client';

import { useEffect, useRef } from 'react';
import { EditorState } from 'prosemirror-state';
import { EditorView } from 'prosemirror-view';
import { Schema } from 'prosemirror-model';
import { baseKeymap } from 'prosemirror-commands';
import { keymap } from 'prosemirror-keymap';
import { history, undo, redo } from 'prosemirror-history';

// Simple schema for text input
const schema = new Schema({
  nodes: {
    doc: {
      content: 'block+',
    },
    paragraph: {
      content: 'inline*',
      group: 'block',
      parseDOM: [{ tag: 'p' }],
      toDOM() {
        return ['p', 0];
      },
    },
    text: {
      group: 'inline',
    },
    hard_break: {
      inline: true,
      group: 'inline',
      selectable: false,
      parseDOM: [{ tag: 'br' }],
      toDOM() {
        return ['br'];
      },
    },
  },
  marks: {},
});

interface ProseMirrorInputProps {
  placeholder?: string;
  onSubmit?: (content: string) => void;
  className?: string;
  disabled?: boolean;
}

export function ProseMirrorInput({
  placeholder = 'Type a message...',
  onSubmit,
  className = '',
  disabled = false,
}: ProseMirrorInputProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!editorRef.current || viewRef.current) return;

    const state = EditorState.create({
      schema,
      plugins: [
        history(),
        keymap({
          'Mod-z': undo,
          'Mod-y': redo,
          'Shift-Mod-z': redo,
        }),
        keymap({
          'Shift-Enter': (state: EditorState, dispatch: any) => {
            if (dispatch) {
              const br = state.schema.nodes.hard_break.create();
              const tr = state.tr.replaceSelectionWith(br);
              dispatch(tr);
            }
            return true;
          },
        }),
        keymap({
          Enter: (state: EditorState, dispatch: any) => {
            // Submit if we have content
            if (onSubmit && !disabled) {
              // Extract text content with line breaks preserved
              let content = '';
              let firstParagraph = true;

              state.doc.forEach((node) => {
                if (node.type.name === 'paragraph') {
                  if (!firstParagraph) {
                    content += '\n';
                  }
                  firstParagraph = false;

                  node.forEach((child) => {
                    if (child.isText) {
                      content += child.text || '';
                    } else if (child.type.name === 'hard_break') {
                      content += '\n';
                    }
                  });
                }
              });

              content = content.trim();

              if (content) {
                onSubmit(content);
                // Clear the editor
                if (dispatch) {
                  const tr = state.tr.delete(0, state.doc.content.size);
                  dispatch(tr);
                }
              }
              return true;
            }
            return false;
          },
        }),
        keymap(baseKeymap),
      ],
    });

    viewRef.current = new EditorView(editorRef.current, {
      state,
      attributes: {
        class: 'prosemirror-input',
        'data-placeholder': placeholder,
        spellcheck: 'false',
      },
      editable: () => !disabled,
    });

    // Auto-focus the editor if not disabled
    if (!disabled) {
      viewRef.current.focus();
    }

    return () => {
      if (viewRef.current) {
        viewRef.current.destroy();
        viewRef.current = null;
      }
    };
  }, [placeholder, onSubmit, disabled]);

  return (
    <div
      className={`prosemirror-input-container ${className}`}
      data-testid="multimodal-input"
      ref={editorRef}
    />
  );
}