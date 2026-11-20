import {
  Eraser,
  LinkSimple,
  ListBullets,
  ListNumbers,
  Quotes,
  TextB,
  TextItalic,
  TextUnderline,
} from '@phosphor-icons/react';
import { Placeholder } from '@tiptap/extensions';
import { EditorContent, useEditor, useEditorState } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { cn } from '@/lib/cn';
import { normaliseUrl } from './link';

function Tool({
  label,
  hint,
  on,
  onClick,
  children,
}: {
  label: string;
  hint?: string;
  on?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={on}
      title={hint ? `${label} (${hint})` : label}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
      className={cn(
        'grid size-7 place-items-center rounded-control text-paper-text-2 transition-colors hover:bg-paper-rule/60',
        on && 'bg-paper-rule/80 text-paper-text',
      )}
    >
      {children}
    </button>
  );
}

export function Editor({
  html,
  onChange,
  onEdit,
  onEscape,
  autoFocus,
}: {
  html: string;
  onChange: (html: string, text: string) => void;
  onEdit: () => void;
  onEscape: () => void;
  autoFocus?: boolean;
}) {
  const [linkOpen, setLinkOpen] = useState(false);
  const [url, setUrl] = useState('');
  const [badUrl, setBadUrl] = useState(false);
  const linkInput = useRef<HTMLInputElement>(null);

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: false,
        code: false,
        codeBlock: false,
        horizontalRule: false,
        strike: false,
        link: { openOnClick: false, autolink: true, defaultProtocol: 'https' },
      }),
      Placeholder.configure({ placeholder: 'Write your message' }),
    ],
    content: html,
    autofocus: autoFocus ? 'start' : false,
    editorProps: {
      attributes: {
        class: 'composer-editor',
        role: 'textbox',
        'aria-multiline': 'true',
        'aria-label': 'Message',
      },
      handleDOMEvents: {
        beforeinput: () => {
          onEdit();
          return false;
        },
      },
      handleKeyDown: (_view, event) => {
        if (event.key === 'Escape') {
          onEscape();
          return true;
        }
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
          event.preventDefault();
          event.stopPropagation();
          setLinkOpen(true);
          return true;
        }
        return false;
      },
    },
    onUpdate: ({ editor: e }) => onChange(e.getHTML(), e.getText()),
  });

  const state = useEditorState({
    editor,
    selector: ({ editor: e }) => ({
      bold: e?.isActive('bold') ?? false,
      italic: e?.isActive('italic') ?? false,
      underline: e?.isActive('underline') ?? false,
      bullets: e?.isActive('bulletList') ?? false,
      numbers: e?.isActive('orderedList') ?? false,
      quote: e?.isActive('blockquote') ?? false,
      link: e?.isActive('link') ?? false,
    }),
  });

  useEffect(() => {
    if (linkOpen) {
      setUrl(editor?.getAttributes('link').href ?? '');
      setBadUrl(false);
      linkInput.current?.focus();
    }
  }, [linkOpen, editor]);

  if (!editor) return <div className="composer-editor" aria-busy="true" />;

  const chain = () => {
    onEdit();
    return editor.chain().focus();
  };

  function applyLink() {
    const href = normaliseUrl(url);
    if (!href) {
      setBadUrl(true);
      return;
    }
    chain().extendMarkRange('link').setLink({ href }).run();
    setLinkOpen(false);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="scroll-y min-h-0 flex-1">
        <EditorContent editor={editor} />
      </div>
      {linkOpen && (
        <form
          className="flex items-center gap-2 border-t border-paper-rule px-3 py-1.5"
          onSubmit={(e) => {
            e.preventDefault();
            applyLink();
          }}
        >
          <label htmlFor="composer-link" className="text-meta text-paper-text-2">
            Link to
          </label>
          <input
            id="composer-link"
            ref={linkInput}
            value={url}
            onChange={(e) => {
              setUrl(e.target.value);
              setBadUrl(false);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.stopPropagation();
                setLinkOpen(false);
                editor.chain().focus().run();
              }
            }}
            placeholder="example.com"
            aria-invalid={badUrl}
            className="h-7 min-w-0 flex-1 rounded-control bg-paper-rule/40 px-2 text-ui text-paper-text outline-none focus-visible:outline-2 focus-visible:outline-paper-blue"
          />
          {badUrl && <span className="text-meta text-red">Not a web or mail address</span>}
          <button
            type="submit"
            className="h-7 rounded-control bg-paper-blue px-3 text-ui font-medium text-on-paper-blue"
          >
            Add link
          </button>
          {state?.link && (
            <button
              type="button"
              className="h-7 rounded-control px-2 text-ui text-paper-text-2 hover:bg-paper-rule/60"
              onClick={() => {
                chain().extendMarkRange('link').unsetLink().run();
                setLinkOpen(false);
              }}
            >
              Remove link
            </button>
          )}
        </form>
      )}
      <div
        role="toolbar"
        aria-label="Formatting"
        className="flex items-center gap-0.5 border-t border-paper-rule px-2 py-1"
      >
        <Tool
          label="Bold"
          hint="Ctrl+B"
          on={state?.bold}
          onClick={() => chain().toggleBold().run()}
        >
          <TextB size={16} weight="bold" />
        </Tool>
        <Tool
          label="Italic"
          hint="Ctrl+I"
          on={state?.italic}
          onClick={() => chain().toggleItalic().run()}
        >
          <TextItalic size={16} />
        </Tool>
        <Tool
          label="Underline"
          hint="Ctrl+U"
          on={state?.underline}
          onClick={() => chain().toggleUnderline().run()}
        >
          <TextUnderline size={16} />
        </Tool>
        <span className="mx-1 h-4 w-px bg-paper-rule" aria-hidden="true" />
        <Tool
          label="Bulleted list"
          on={state?.bullets}
          onClick={() => chain().toggleBulletList().run()}
        >
          <ListBullets size={16} />
        </Tool>
        <Tool
          label="Numbered list"
          on={state?.numbers}
          onClick={() => chain().toggleOrderedList().run()}
        >
          <ListNumbers size={16} />
        </Tool>
        <Tool label="Quote" on={state?.quote} onClick={() => chain().toggleBlockquote().run()}>
          <Quotes size={16} />
        </Tool>
        <Tool label="Link" hint="Ctrl+K" on={state?.link} onClick={() => setLinkOpen((o) => !o)}>
          <LinkSimple size={16} />
        </Tool>
        <span className="mx-1 h-4 w-px bg-paper-rule" aria-hidden="true" />
        <Tool label="Clear formatting" onClick={() => chain().unsetAllMarks().clearNodes().run()}>
          <Eraser size={16} />
        </Tool>
      </div>
    </div>
  );
}
