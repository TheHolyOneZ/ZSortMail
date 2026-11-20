import {
  Archive,
  ArrowBendUpLeft,
  ArrowLeft,
  CaretDown,
  CaretUp,
  EnvelopeSimple,
  EyeSlash,
  ImageBroken,
  Paperclip,
  ShieldCheck,
  Star,
  Trash,
} from '@phosphor-icons/react';
import { useNavigate } from '@tanstack/react-router';
import { animate, stagger } from 'motion';
import {
  type CSSProperties,
  Fragment,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { MessageView, ParticipantRef } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { composeFor, writeToPerson } from '@/lib/composer';
import {
  formatMessageTime,
  formatSize,
  gapLabel,
  isLongGap,
  plural,
  spanLabel,
} from '@/lib/format';
import { stampColor } from '@/lib/ink';
import { useConversation, useConversations } from '@/lib/queries';
import { useActions, useShortcuts } from '@/lib/shortcuts';
import { toastError } from '@/lib/toasts';
import { Postmark } from '@/motion/choreographies/stamp';
import { useMotionGovernor } from '@/motion/MotionProvider';
import { gapPx, gapsBetween } from '@/motion/strand';
import { ease } from '@/motion/tokens';
import { Button, IconButton, Kbd, Segmented } from '@/ui/controls';
import { ZMark } from '@/ui/icons';
import { Menu } from '@/ui/Menu';
import { Stamp } from '@/ui/Stamp';
import { type PreviewItem, QuickLook } from '../files/QuickLook';
import { SnoozeMenu } from '../today/SnoozeMenu';
import { Letter } from './Letter';
import { useLinkGuard } from './LinkGuard';
import { MailFrame } from './MailFrame';
import { MessageDetails } from './MessageDetails';
import { TagBar } from './TagBar';
import { UnsubscribeRow } from './Unsubscribe';

export function StrandView({
  conversationId,
  personId,
  onBack,
}: {
  conversationId?: number;
  personId?: number;
  onBack?: () => void;
}) {
  if (conversationId === undefined) {
    return (
      <section className="grid h-full place-items-center bg-desk px-8" aria-label="Conversation">
        <div className="text-center">
          <ZMark size={44} className="mx-auto text-text-3" />
          <p className="mt-3 text-ui text-text-2">Open a conversation to read it.</p>
        </div>
      </section>
    );
  }
  return (
    <StrandContent
      key={conversationId}
      conversationId={conversationId}
      personId={personId}
      onBack={onBack}
    />
  );
}

function shortName(name: string): string {
  return name.replace(/\s*[([].*$/u, '').trim();
}

function participantsLine(people: ParticipantRef[]): string {
  const names = people.map((p) => shortName(p.name));
  if (names.length === 0) return 'You';
  if (names.length === 1) return `${names[0]} and you`;
  return `${names.slice(0, -1).join(', ')}, ${names[names.length - 1]} and you`;
}

function StrandContent({
  conversationId,
  personId,
  onBack,
}: {
  conversationId: number;
  personId?: number;
  onBack?: () => void;
}) {
  const { data: convo } = useConversation(conversationId, personId);
  const { spec } = useMotionGovernor();
  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [toggled, setToggled] = useState<Set<number>>(() => new Set());
  const [view, setView] = useState<'letter' | 'original'>('letter');
  const [remote, setRemote] = useState<Set<number>>(() => new Set());
  const [preview, setPreview] = useState<number | null>(null);
  const linkGuard = useLinkGuard();
  const navigate = useNavigate();
  const { data: siblings = [] } = useConversations(personId);

  function leave() {
    if (personId === undefined) return;
    const at = siblings.findIndex((c) => c.id === conversationId);
    const next = siblings[at + 1] ?? siblings[at - 1];
    if (next) {
      void navigate({
        to: '/people/$personId/c/$conversationId',
        params: { personId: String(personId), conversationId: String(next.id) },
      });
    } else {
      void navigate({ to: '/people/$personId', params: { personId: String(personId) } });
    }
  }
  const archive = () => void act(() => api.archiveConversation(conversationId, true));
  const trash = () =>
    void act(() => api.trashConversation(conversationId, true)).then((ok) => {
      if (ok) leave();
    });
  const hide = () =>
    void act(() => api.hideConversation(conversationId, true)).then((ok) => {
      if (ok) leave();
    });
  const markUnread = () => void api.markConversationRead(conversationId, false).catch(toastError);
  const snooze = () => {
    const d = new Date(now() + 86_400_000);
    d.setHours(8, 0, 0, 0);
    void act(() => api.snoozeConversation(conversationId, d.getTime())).then((ok) => {
      if (ok) leave();
    });
  };
  useActions({
    archive,
    trash,
    hide,
    snooze,
    unread: markUnread,
    reply: () => void composeFor(conversationId, 'reply'),
    reply_all: () => void composeFor(conversationId, 'reply_all'),
    forward: () => void composeFor(conversationId, 'forward'),
  });
  useShortcuts([{ key: 'Delete', run: trash }]);

  const messages = useMemo(() => convo?.messages ?? [], [convo]);
  const hasFormatted = messages.some((m) => m.html_url);

  const previewItems = useMemo<PreviewItem[]>(
    () =>
      messages.flatMap((m) =>
        m.attachments
          .filter((a) => !a.inline)
          .map((a) => ({
            id: a.id,
            name: a.name,
            mime: a.mime,
            size: a.size,
            caption: `${m.from.is_me ? 'You' : shortName(m.from.name)}, ${formatMessageTime(m.sent_at, now())}`,
          })),
      ),
    [messages],
  );

  useEffect(() => {
    void api.markConversationSeen(conversationId).catch(() => {});
  }, [conversationId]);

  const unreadCount = convo?.summary.unread ?? 0;
  useEffect(() => {
    if (unreadCount === 0) return;
    const timer = setTimeout(() => {
      api.markConversationRead(conversationId, true).catch(toastError);
    }, 1500);
    return () => clearTimeout(timer);
  }, [unreadCount, conversationId]);
  const gaps = useMemo(() => gapsBetween(messages.map((m) => m.sent_at)), [messages]);

  const openByDefault = useMemo(() => {
    const open = new Set<number>();
    messages.forEach((m, i) => {
      if (messages.length <= 4 || i >= messages.length - 2 || m.unread) open.add(m.id);
    });
    return open;
  }, [messages]);
  const isOpen = (id: number) => openByDefault.has(id) !== toggled.has(id);
  const toggle = (id: number) =>
    setToggled((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const loadedId = convo?.summary.id;
  useLayoutEffect(() => {
    const scroller = scrollRef.current;
    const content = rootRef.current;
    if (!scroller || !content || loadedId === undefined) return;

    let pinned = true;
    const toBottom = () => {
      scroller.scrollTop = scroller.scrollHeight;
    };
    toBottom();
    const observer = new ResizeObserver(() => {
      if (pinned) toBottom();
    });
    observer.observe(content);

    const release = () => {
      pinned = false;
    };
    const events = ['wheel', 'touchstart', 'pointerdown', 'keydown'] as const;
    for (const name of events) scroller.addEventListener(name, release, { passive: true });
    return () => {
      observer.disconnect();
      for (const name of events) scroller.removeEventListener(name, release);
    };
  }, [loadedId]);

  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root || !convo || !spec.draw) return;
    const lines = Array.from(root.querySelectorAll<HTMLElement>('.strand-line')).slice(0, 14);
    const knots = Array.from(root.querySelectorAll<HTMLElement>('[data-knot]')).slice(0, 12);
    for (const line of lines) line.style.transform = 'scaleY(0)';
    for (const knot of knots) {
      knot.style.opacity = '0';
      knot.style.transform = 'scale(0.6)';
    }
    const controls = [
      animate(
        lines,
        { transform: ['scaleY(0)', 'scaleY(1)'] },
        {
          duration: 0.42,
          delay: stagger(0.024),
          ease: ease.fold,
        },
      ),
      animate(
        knots,
        { opacity: [0, 1], transform: ['scale(0.6)', 'scale(1)'] },
        {
          duration: 0.24,
          delay: stagger(0.024),
          ease: ease.stamp,
        },
      ),
    ];
    return () => {
      for (const control of controls) control.cancel();
      for (const el of [...lines, ...knots]) {
        el.style.transform = '';
        el.style.opacity = '';
      }
    };
  }, [convo, spec.draw]);

  if (!convo) return <section className="h-full bg-desk" aria-label="Conversation" />;

  const first = messages[0];
  const last = messages[messages.length - 1];
  const span = first && last ? last.sent_at - first.sent_at : 0;
  const replyTo = convo.participants.map((p) => shortName(p.name)).join(', ');

  return (
    <section className="flex h-full min-w-0 flex-col bg-desk" aria-label="Conversation">
      <header className="shrink-0 px-8 pt-6 pb-4">
        {onBack && (
          <IconButton label="Back to conversations" className="-ml-2 mb-2" onClick={onBack}>
            <ArrowLeft size={18} />
          </IconButton>
        )}
        <h2 dir="auto" className="line-clamp-2 text-title font-semibold tracking-[-0.01em]">
          {convo.summary.subject}
        </h2>
        <div className="mt-1 flex items-center gap-3">
          <p className="min-w-0 flex-1 text-ui text-text-2">
            {participantsLine(convo.participants)}. {plural(messages.length, 'message')} over{' '}
            {spanLabel(span)}.
          </p>
          {hasFormatted && (
            <Segmented
              label="How to show messages"
              value={view}
              onChange={setView}
              options={[
                { value: 'letter', label: 'Letter' },
                { value: 'original', label: 'Original' },
              ]}
            />
          )}
          <div className="flex items-center">
            <Menu
              label="Reply, reply to all or forward"
              trigger={<ArrowBendUpLeft size={18} />}
              items={[
                {
                  label: 'Reply',
                  hint: 'R',
                  onSelect: () => void composeFor(conversationId, 'reply'),
                },
                {
                  label: 'Reply to all',
                  hint: 'A',
                  onSelect: () => void composeFor(conversationId, 'reply_all'),
                },
                {
                  label: 'Forward',
                  hint: 'F',
                  onSelect: () => void composeFor(conversationId, 'forward'),
                },
                ...(personId === undefined
                  ? []
                  : [
                      {
                        label: 'Write a new message',
                        separated: true,
                        hint: 'C',
                        onSelect: () => void writeToPerson(personId, replyTo),
                      },
                    ]),
              ]}
            />
            <IconButton label="Archive" onClick={archive}>
              <Archive size={18} />
            </IconButton>
            <IconButton label="Move to trash" onClick={trash}>
              <Trash size={18} />
            </IconButton>
            <SnoozeMenu conversationId={conversationId} />
            <IconButton label="Hide conversation" onClick={hide}>
              <EyeSlash size={18} />
            </IconButton>
            <IconButton label="Mark as unread" onClick={markUnread}>
              <EnvelopeSimple size={18} />
            </IconButton>
          </div>
        </div>
        <TagBar conversationId={conversationId} />
      </header>

      <div ref={scrollRef} className="scroll-y min-h-0 flex-1 px-6 pb-8">
        <div ref={rootRef} className="strand mx-auto max-w-[820px]">
          {messages.map((m, i) => (
            <Fragment key={m.id}>
              {i > 0 && <GapRow gapMs={gaps[i] ?? 0} />}
              <MessageRow
                message={m}
                open={isOpen(m.id)}
                onToggle={() => toggle(m.id)}
                original={view === 'original'}
                allowRemote={remote.has(m.id)}
                onAllowRemote={(allow) =>
                  setRemote((prev) => {
                    const next = new Set(prev);
                    if (allow) next.add(m.id);
                    else next.delete(m.id);
                    return next;
                  })
                }
                onLink={linkGuard.request}
                onOpenAttachment={(attachmentId) => {
                  const at = previewItems.findIndex((item) => item.id === attachmentId);
                  if (at >= 0) setPreview(at);
                }}
              />
            </Fragment>
          ))}
          <ReplyRow conversationId={convo.summary.id} replyTo={replyTo} />
        </div>
      </div>
      {linkGuard.dialog}
      <QuickLook
        items={previewItems}
        index={preview}
        onIndex={setPreview}
        onClose={() => setPreview(null)}
      />
    </section>
  );
}

function GapRow({ gapMs }: { gapMs: number }) {
  const long = isLongGap(gapMs);
  return (
    <div
      className="strand-row"
      data-long={long}
      style={{ height: gapPx(gapMs), '--line-top': '0px' } as CSSProperties}
    >
      <div className="strand-gutter">
        <span className="strand-line" />
      </div>
      <div className="flex items-center gap-3 ps-3">
        <span className={cn('text-meta text-text-2', long && 'font-medium')}>
          {gapLabel(gapMs)}
        </span>
        {long && <span className="h-px flex-1 border-t border-dashed border-rule-strong" />}
      </div>
    </div>
  );
}

function MessageRow({
  message: m,
  open,
  onToggle,
  original,
  allowRemote,
  onAllowRemote,
  onLink,
  onOpenAttachment,
}: {
  message: MessageView;
  open: boolean;
  onToggle: () => void;
  original: boolean;
  allowRemote: boolean;
  onAllowRemote: (allow: boolean) => void;
  onLink: (href: string, text: string) => void;
  onOpenAttachment: (attachmentId: number) => void;
}) {
  const [showQuoted, setShowQuoted] = useState(false);
  const [details, setDetails] = useState(false);
  const nowTs = now();
  const time = formatMessageTime(m.sent_at, nowTs);
  const rowStyle = { '--line-top': open ? '40px' : '30px' } as CSSProperties;

  return (
    <div className="strand-row skip-offscreen" style={rowStyle}>
      <div className="strand-gutter pt-0.5">
        <span data-knot className="relative z-10 block origin-center">
          <Stamp name={m.from.name} ink={m.from.ink} kind={m.from.kind} size={open ? 32 : 24} />
        </span>
        <span className="strand-line" />
      </div>

      {open ? (
        <article
          className="sheet min-w-0"
          data-mine={m.from.is_me}
          data-unread={m.unread}
          style={{ '--stamp': stampColor(m.from.ink) } as CSSProperties}
        >
          <div className="flex items-center gap-1 pe-2">
            <button
              type="button"
              onClick={onToggle}
              aria-expanded="true"
              className="flex min-w-0 flex-1 items-baseline gap-2 rounded-tl-sheet px-4 pt-3 pb-1 text-left"
            >
              <span dir="auto" className="truncate text-body font-semibold">
                {shortName(m.from.name)}
              </span>
              <span dir="auto" className="min-w-0 truncate text-ui text-paper-text-2">
                {m.to_line}
              </span>
              <time className="tabular ml-auto shrink-0 text-meta text-paper-text-2">{time}</time>
              <CaretUp
                size={12}
                className="shrink-0 self-center text-paper-text-2"
                aria-hidden="true"
              />
            </button>
            <button
              type="button"
              aria-label={m.starred ? 'Remove star' : 'Star this message'}
              aria-pressed={m.starred}
              onClick={() => api.setMessageStarred(m.id, !m.starred).catch(toastError)}
              className="mt-2 grid size-7 shrink-0 place-items-center rounded-control text-paper-text-2 hover:bg-black/5 hover:text-paper-text"
            >
              <Star
                size={16}
                weight={m.starred ? 'fill' : 'regular'}
                className={m.starred ? 'text-[#b7791f]' : ''}
              />
            </button>
          </div>

          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 pb-2">
            <span
              dir="ltr"
              className="min-w-0 truncate rounded-[4px] bg-black/[0.05] px-2 py-0.5 text-ui font-medium text-paper-text select-text"
              title={m.from_email}
            >
              {m.from_email}
            </span>
            <button
              type="button"
              aria-expanded={details}
              onClick={() => setDetails((v) => !v)}
              className="shrink-0 rounded-control border border-paper-rule px-2 py-0.5 text-meta font-medium text-paper-text-2 transition-colors hover:bg-black/5 hover:text-paper-text"
            >
              {details ? 'Hide details' : 'Details'}
            </button>
          </div>
          {details && <MessageDetails message={m} />}

          <div className="px-4 pt-1 pb-4">
            {original && m.html_url ? (
              <>
                <FrameNotes message={m} allowRemote={allowRemote} onAllowRemote={onAllowRemote} />
                <MailFrame message={m} allowRemote={allowRemote} onLink={onLink} />
              </>
            ) : (
              <Letter text={m.text} />
            )}

            {m.quoted && !(original && m.html_url) && (
              <div className="mt-3">
                <button
                  type="button"
                  aria-expanded={showQuoted}
                  onClick={() => setShowQuoted((v) => !v)}
                  className="inline-flex items-center gap-1 rounded-sheet text-meta font-medium text-paper-text-2 hover:text-paper-text"
                >
                  <CaretDown
                    size={11}
                    className={cn('transition-transform', showQuoted && 'rotate-180')}
                  />
                  {showQuoted ? 'Hide quoted text' : 'Show quoted text'}
                </button>
                {showQuoted && (
                  <blockquote dir="auto" className="quoted mt-2 text-ui whitespace-pre-line">
                    {m.quoted.replace(/^>\s?/gm, '')}
                  </blockquote>
                )}
              </div>
            )}

            {m.unsubscribe && <UnsubscribeRow message={m} name={shortName(m.from.name)} />}

            {m.attachments.some((a) => !a.inline) && (
              <ul className="mt-3 flex flex-wrap gap-2" aria-label="Attachments">
                {m.attachments
                  .filter((a) => !a.inline)
                  .map((a) => (
                    <li key={a.id}>
                      <button
                        type="button"
                        onClick={() => onOpenAttachment(a.id)}
                        className="inline-flex max-w-full items-center gap-2 rounded-sheet border border-paper-rule px-2.5 py-1.5 text-ui transition-colors hover:bg-black/5"
                      >
                        <Paperclip size={14} className="shrink-0 text-paper-text-2" />
                        <span className="truncate">{a.name}</span>
                        <span className="tabular shrink-0 text-meta text-paper-text-2">
                          {formatSize(a.size)}
                        </span>
                      </button>
                    </li>
                  ))}
              </ul>
            )}
          </div>
        </article>
      ) : (
        <button
          type="button"
          onClick={onToggle}
          aria-expanded="false"
          className="flex min-w-0 items-center gap-3 rounded-sheet px-3 py-1.5 text-left transition-colors hover:bg-hover"
        >
          <span dir="auto" className="shrink-0 text-ui font-medium">
            {shortName(m.from.name)}
          </span>
          <span dir="auto" className="min-w-0 flex-1 truncate text-ui text-text-2">
            {m.preview}
          </span>
          <time className="tabular shrink-0 text-meta text-text-2">{time}</time>
        </button>
      )}
    </div>
  );
}

function FrameNotes({
  message: m,
  allowRemote,
  onAllowRemote,
}: {
  message: MessageView;
  allowRemote: boolean;
  onAllowRemote: (allow: boolean) => void;
}) {
  if (m.remote_images === 0 && m.trackers_blocked === 0) return null;
  return (
    <div className="mb-3 flex flex-wrap items-center gap-x-4 gap-y-1 rounded-sheet bg-desk/70 px-3 py-2 text-ui text-paper-text-2">
      {m.remote_images > 0 && (
        <span className="inline-flex items-center gap-2">
          <ImageBroken size={16} aria-hidden="true" />
          {allowRemote
            ? `Showing ${plural(m.remote_images, 'remote image')}.`
            : `${plural(m.remote_images, 'remote image')} not loaded.`}
          <button
            type="button"
            onClick={() => onAllowRemote(!allowRemote)}
            className="font-semibold text-paper-blue underline-offset-2 hover:underline"
          >
            {allowRemote ? 'Hide images' : 'Load images'}
          </button>
        </span>
      )}
      {m.trackers_blocked > 0 && (
        <span className="inline-flex items-center gap-2">
          <ShieldCheck size={16} aria-hidden="true" />
          {plural(m.trackers_blocked, 'tracker')} blocked.
        </span>
      )}
    </div>
  );
}

const isMac = typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform);

function ReplyRow({ conversationId, replyTo }: { conversationId: number; replyTo: string }) {
  const [text, setText] = useState('');
  const [sending, setSending] = useState(false);
  const [stamped, setStamped] = useState(false);
  const field = useRef<HTMLTextAreaElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);

  function autosize() {
    const el = field.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 320)}px`;
  }

  async function send() {
    const body = text.trim();
    if (!body || sending) return;
    setSending(true);
    try {
      await api.reply(conversationId, body);
      setText('');
      requestAnimationFrame(autosize);
      setStamped(true);
      requestAnimationFrame(() =>
        rowRef.current?.scrollIntoView({ block: 'end', behavior: 'smooth' }),
      );
    } finally {
      setSending(false);
    }
  }

  return (
    <div ref={rowRef} className="strand-row mt-3">
      <div className="strand-gutter pt-0.5">
        <Stamp name="You" ink={0} kind="self" size={32} />
      </div>
      <div className="sheet relative min-w-0">
        <label className="block">
          <span className="sr-only">Reply to {replyTo}</span>
          <textarea
            ref={field}
            value={text}
            rows={2}
            placeholder={`Reply to ${replyTo}`}
            onChange={(e) => {
              setText(e.target.value);
              autosize();
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                void send();
              }
            }}
            className="letter block w-full resize-none bg-transparent px-4 pt-3 pb-2 text-paper-text outline-none placeholder:text-paper-text-2"
          />
        </label>
        <div className="flex items-center justify-end gap-3 px-3 pb-3">
          <button
            type="button"
            onClick={() => void composeFor(conversationId, 'reply', text)}
            className="rounded-control px-2 py-1 text-ui text-paper-text-2 hover:bg-paper-rule/60 hover:text-paper-text"
          >
            Open in composer
          </button>
          <span className="inline-flex items-center gap-1 text-meta text-paper-text-2">
            <Kbd tone="paper">{isMac ? '⌘' : 'Ctrl'}</Kbd>
            <Kbd tone="paper">Enter</Kbd>
          </span>
          <Button
            variant="paperPrimary"
            disabled={!text.trim() || sending}
            onClick={() => void send()}
          >
            Send
          </Button>
        </div>
        {stamped && (
          <div className="absolute -top-9 right-4 z-10">
            <Postmark label="Sent" onDone={() => setStamped(false)} />
          </div>
        )}
      </div>
    </div>
  );
}
