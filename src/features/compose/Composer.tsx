import { BellSimple, CaretDown, Clock, Minus, Paperclip, Trash, X } from '@phosphor-icons/react';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useCallback, useEffect, useRef, useState } from 'react';
import { addressLabel } from '@/lib/addresses';
import { api } from '@/lib/api';
import type { Address, Draft, DraftInput } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { useComposer } from '@/lib/composer';
import { formatSize } from '@/lib/format';
import { useComposeSettings } from '@/lib/queries';
import { snoozeChoices } from '@/lib/snooze';
import { toast, toastError, useToasts } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Menu } from '@/ui/Menu';
import { Modal } from '@/ui/Modal';
import { Editor } from './Editor';
import { RecipientField } from './RecipientField';

const MAX_BYTES = 25 * 1024 * 1024;

const REMINDERS: { label: string; days: number | null }[] = [
  { label: 'Do not remind me', days: null },
  { label: 'If no reply in 2 days', days: 2 },
  { label: 'If no reply in 3 days', days: 3 },
  { label: 'If no reply in 1 week', days: 7 },
  { label: 'If no reply in 2 weeks', days: 14 },
];

function reminderLabel(days: number | null): string {
  if (days === null) return '';
  return days % 7 === 0
    ? `Reminder in ${days / 7} ${days === 7 ? 'week' : 'weeks'}`
    : `Reminder in ${days} days`;
}

function readBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const text = String(reader.result);
      resolve(text.slice(text.indexOf(',') + 1));
    };
    reader.onerror = () => reject(reader.error ?? new Error('That file could not be read'));
    reader.readAsDataURL(file);
  });
}

function when(at: number): string {
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(at);
}

function localInputValue(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function Composer() {
  const draftId = useComposer((s) => s.draftId);
  if (draftId === null) return null;
  return <Loader key={draftId} id={draftId} />;
}

function Loader({ id }: { id: number }) {
  const initial = useComposer((s) => (s.initial?.id === id ? s.initial : undefined));
  const { data } = useQuery({
    queryKey: ['draft', id],
    queryFn: () => api.getDraft(id),
    gcTime: 0,
    initialData: initial,
  });
  useEffect(() => {
    if (data === null) useComposer.getState().close();
  }, [data]);
  if (!data) return null;
  return <Form draft={data} />;
}

function title(draft: Draft, to: Address[]): string {
  const first = to[0];
  switch (draft.mode) {
    case 'reply':
      return first ? `Reply to ${addressLabel(first)}` : 'Reply';
    case 'reply_all':
      return 'Reply to all';
    case 'forward':
      return 'Forward';
    default:
      return 'New message';
  }
}

function Form({ draft }: { draft: Draft }) {
  const navigate = useNavigate();
  const minimized = useComposer((s) => s.minimized);
  const focus = useComposer((s) => s.focus);
  const { data: settings } = useComposeSettings();

  const [to, setTo] = useState(draft.to);
  const [cc, setCc] = useState(draft.cc);
  const [bcc, setBcc] = useState(draft.bcc);
  const [showCc, setShowCc] = useState(draft.cc.length > 0);
  const [showBcc, setShowBcc] = useState(draft.bcc.length > 0);
  const [subject, setSubject] = useState(draft.subject);
  const [bodyHtml, setBodyHtml] = useState(draft.body_html);
  const [nudge, setNudge] = useState<number | null>(draft.nudge_days);
  const [files, setFiles] = useState(draft.attachments);
  const [scheduled, setScheduled] = useState(draft.send_at);
  const [status, setStatus] = useState<'idle' | 'saving' | 'saved' | 'failed'>('idle');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [picking, setPicking] = useState(false);
  const [pickedTime, setPickedTime] = useState(() => localInputValue(now() + 86_400_000));
  const picker = useRef<HTMLInputElement>(null);

  const changed = useRef(false);
  const gone = useRef(false);
  const latest = useRef<DraftInput | null>(null);
  latest.current = {
    id: draft.id,
    to,
    cc,
    bcc,
    subject,
    body_html: bodyHtml,
    nudge_days: nudge,
  };

  const save = useCallback(async () => {
    const input = latest.current;
    if (!input || gone.current) return;
    setStatus('saving');
    try {
      await api.saveDraft(input);
      setStatus('saved');
    } catch {
      setStatus('failed');
    }
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: the fields are the trigger; `save` reads the latest of them.
  useEffect(() => {
    if (!changed.current) return;
    const timer = setTimeout(() => void save(), 700);
    return () => clearTimeout(timer);
  }, [to, cc, bcc, subject, bodyHtml, nudge, save]);

  useEffect(
    () => () => {
      if (changed.current && !gone.current && latest.current) void api.saveDraft(latest.current);
    },
    [],
  );

  function touch<T>(set: (value: T) => void) {
    return (value: T) => {
      changed.current = true;
      setError(null);
      set(value);
    };
  }

  async function flush() {
    if (changed.current) await save();
  }

  const closeRef = useRef<() => Promise<void>>(async () => {});
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== 'Escape' || e.defaultPrevented || document.querySelector('dialog[open]'))
        return;
      void closeRef.current();
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  async function close() {
    if (!changed.current && !draft.touched) {
      gone.current = true;
      await api.deleteDraft(draft.id).catch(() => {});
      useComposer.getState().close();
      return;
    }
    await flush();
    useComposer.getState().close();
    toast('Draft saved.', {
      label: 'Discard',
      run: () => {
        void api.deleteDraft(draft.id).then(() => toast('Draft discarded.'), toastError);
      },
    });
  }

  closeRef.current = close;

  async function discard() {
    gone.current = true;
    setConfirming(false);
    try {
      await api.deleteDraft(draft.id);
      useComposer.getState().close();
      toast('Draft discarded.');
    } catch (e) {
      gone.current = false;
      toastError(e);
    }
  }

  async function send() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await flush();
      const ticket = await api.sendDraft(draft.id);
      gone.current = true;
      useComposer.getState().close();
      const undoMs = (settings?.undo_seconds ?? 10) * 1000;
      useToasts.getState().push(
        {
          text: 'Sent.',
          tone: 'info',
          action:
            undoMs > 0
              ? {
                  label: 'Undo',
                  run: () => {
                    api
                      .unsend(ticket.message_id)
                      .then((back) => useComposer.getState().show(back.id, 'body'), toastError);
                  },
                }
              : undefined,
        },
        Math.max(undoMs, 4000),
      );
      if ((draft.mode === 'new' || draft.mode === 'forward') && ticket.person_id !== null) {
        void navigate({
          to: '/people/$personId/c/$conversationId',
          params: {
            personId: String(ticket.person_id),
            conversationId: String(ticket.conversation_id),
          },
        });
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String((e as { message?: string }).message ?? e));
      setBusy(false);
    }
  }

  async function schedule(at: number) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await flush();
      await api.scheduleDraft(draft.id, at);
      gone.current = true;
      useComposer.getState().close();
      toast(`Scheduled for ${when(at)}.`, {
        label: 'Undo',
        run: () => {
          api
            .scheduleDraft(draft.id, null)
            .then(() => useComposer.getState().show(draft.id, 'body'), toastError);
        },
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String((e as { message?: string }).message ?? e));
      setBusy(false);
    }
  }

  async function attach(list: FileList | File[]) {
    for (const file of Array.from(list)) {
      if (file.size > MAX_BYTES) {
        setError(`“${file.name}” is over 25 MB, which is more than Gmail accepts`);
        continue;
      }
      try {
        const added = await api.addDraftFile(
          draft.id,
          file.name,
          file.type,
          await readBase64(file),
        );
        changed.current = true;
        setFiles((prev) => [...prev, added]);
      } catch (e) {
        setError(String((e as { message?: string }).message ?? e));
      }
    }
  }

  const choices = snoozeChoices(now());

  return (
    <section
      aria-label={title(draft, to)}
      data-dragging={dragging}
      className={cn(
        'composer fixed bottom-0 z-30 flex flex-col overflow-hidden',
        'end-6 w-[min(640px,calc(100vw-24px))]',
        minimized ? 'h-auto' : 'max-h-[min(78vh,720px)] min-h-[420px]',
      )}
      onKeyDown={(e) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          void send();
        }
      }}
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes('Files')) {
          e.preventDefault();
          setDragging(true);
        }
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDragging(false);
      }}
      onDrop={(e) => {
        setDragging(false);
        if (e.dataTransfer.files.length > 0) {
          e.preventDefault();
          void attach(e.dataTransfer.files);
        }
      }}
      onPaste={(e) => {
        if (e.clipboardData.files.length > 0) {
          e.preventDefault();
          void attach(e.clipboardData.files);
        }
      }}
    >
      <div className="composer-stripe" aria-hidden="true" />
      <header className="flex items-center gap-2 border-b border-paper-rule px-4 py-2">
        <h2 dir="auto" className="min-w-0 flex-1 truncate text-ui font-semibold">
          {subject.trim() || title(draft, to)}
        </h2>
        <button
          type="button"
          aria-label={minimized ? 'Expand' : 'Minimize'}
          title={minimized ? 'Expand' : 'Minimize'}
          onClick={() => useComposer.getState().setMinimized(!minimized)}
          className="grid size-7 place-items-center rounded-control text-paper-text-2 hover:bg-paper-rule/60"
        >
          <Minus size={16} />
        </button>
        <button
          type="button"
          aria-label="Close"
          title="Close (Esc)"
          onClick={() => void close()}
          className="grid size-7 place-items-center rounded-control text-paper-text-2 hover:bg-paper-rule/60"
        >
          <X size={16} />
        </button>
      </header>

      <div className={cn('flex min-h-0 flex-1 flex-col', minimized && 'hidden')}>
        {scheduled !== null && (
          <div className="flex items-center gap-3 border-b border-paper-rule bg-paper-rule/30 px-4 py-1.5 text-ui">
            <Clock size={15} />
            <span className="flex-1">Scheduled for {when(scheduled)}.</span>
            <button
              type="button"
              className="font-medium text-paper-blue underline underline-offset-2"
              onClick={() =>
                void api.scheduleDraft(draft.id, null).then(() => setScheduled(null), toastError)
              }
            >
              Cancel schedule
            </button>
          </div>
        )}
        <RecipientField
          label="To"
          value={to}
          onChange={touch(setTo)}
          autoFocus={focus === 'to'}
          trailing={
            <span className="flex gap-1 pt-1 text-meta text-paper-text-2">
              {!showCc && (
                <button
                  type="button"
                  className="rounded-[4px] px-1 hover:bg-paper-rule/60"
                  onClick={() => setShowCc(true)}
                >
                  Cc
                </button>
              )}
              {!showBcc && (
                <button
                  type="button"
                  className="rounded-[4px] px-1 hover:bg-paper-rule/60"
                  onClick={() => setShowBcc(true)}
                >
                  Bcc
                </button>
              )}
            </span>
          }
        />
        {showCc && <RecipientField label="Cc" value={cc} onChange={touch(setCc)} />}
        {showBcc && <RecipientField label="Bcc" value={bcc} onChange={touch(setBcc)} />}
        <div className="flex items-center gap-2 border-b border-paper-rule px-4 py-1.5">
          <label htmlFor="composer-subject" className="w-14 shrink-0 text-meta text-paper-text-2">
            Subject
          </label>
          <input
            id="composer-subject"
            value={subject}
            dir="auto"
            onChange={(e) => touch(setSubject)(e.target.value)}
            className="h-6 min-w-0 flex-1 bg-transparent text-ui text-paper-text outline-none"
          />
        </div>

        <Editor
          html={bodyHtml}
          autoFocus={focus === 'body'}
          onChange={(html) => setBodyHtml(html)}
          onEdit={() => {
            changed.current = true;
            setError(null);
          }}
          onEscape={() => void closeRef.current()}
        />

        {files.length > 0 && (
          <ul
            aria-label="Files to send"
            className="flex flex-wrap gap-1.5 border-t border-paper-rule px-3 py-2"
          >
            {files.map((f) => (
              <li
                key={f.id}
                className="flex h-7 max-w-[240px] items-center gap-1.5 rounded-[4px] bg-paper-rule/40 ps-2 pe-0.5 text-ui"
              >
                <Paperclip size={13} className="shrink-0 text-paper-text-2" />
                <span dir="auto" className="truncate">
                  {f.filename}
                </span>
                <span className="shrink-0 text-meta text-paper-text-2">{formatSize(f.size)}</span>
                <button
                  type="button"
                  aria-label={`Remove ${f.filename}`}
                  onClick={() =>
                    void api
                      .removeDraftFile(f.id)
                      .then(() => setFiles((prev) => prev.filter((x) => x.id !== f.id)), toastError)
                  }
                  className="grid size-6 place-items-center rounded-[4px] text-paper-text-2 hover:bg-paper-rule"
                >
                  <X size={11} weight="bold" />
                </button>
              </li>
            ))}
          </ul>
        )}

        <footer className="flex items-center gap-1 border-t border-paper-rule px-3 py-2">
          <Button variant="paperPrimary" disabled={busy} onClick={() => void send()}>
            Send
          </Button>
          <Menu
            label="Send later"
            side="up"
            align="start"
            trigger={<Clock size={17} />}
            triggerClassName="text-paper-text-2 hover:bg-paper-rule/60 hover:text-paper-text"
            items={[
              ...choices.map((c) => ({
                label: c.label,
                hint: when(c.at),
                onSelect: () => void schedule(c.at),
              })),
              { label: 'Pick a time', separated: true, onSelect: () => setPicking(true) },
            ]}
          />
          <input
            ref={picker}
            type="file"
            multiple
            hidden
            aria-label="Choose files"
            onChange={(e) => {
              if (e.target.files) void attach(e.target.files);
              e.target.value = '';
            }}
          />
          <button
            type="button"
            aria-label="Attach files"
            title="Attach files"
            onClick={() => picker.current?.click()}
            className="grid size-8 place-items-center rounded-control text-paper-text-2 hover:bg-paper-rule/60 hover:text-paper-text"
          >
            <Paperclip size={17} />
          </button>
          <Menu
            label="Remind me if no reply"
            side="up"
            align="start"
            trigger={
              <span className="flex items-center gap-0.5">
                <BellSimple size={17} weight={nudge === null ? 'regular' : 'fill'} />
                <CaretDown size={9} />
              </span>
            }
            triggerClassName="w-11 text-paper-text-2 hover:bg-paper-rule/60 hover:text-paper-text"
            items={REMINDERS.map((r) => ({
              label: r.label,
              onSelect: () => touch(setNudge)(r.days),
            }))}
          />
          {nudge !== null && (
            <span className="text-meta text-paper-text-2">{reminderLabel(nudge)}</span>
          )}
          <span className="flex-1" />
          <span
            role="status"
            aria-live="polite"
            className={cn('text-meta', status === 'failed' ? 'text-red' : 'text-paper-text-2')}
          >
            {status === 'saving' && 'Saving'}
            {status === 'saved' && 'Draft saved'}
            {status === 'failed' && 'Could not save the draft'}
          </span>
          <button
            type="button"
            aria-label="Discard draft"
            title="Discard draft"
            onClick={() => setConfirming(true)}
            className="grid size-8 place-items-center rounded-control text-paper-text-2 hover:bg-paper-rule/60 hover:text-red"
          >
            <Trash size={17} />
          </button>
        </footer>
        {error && (
          <p role="alert" className="border-t border-paper-rule px-4 py-2 text-ui text-red">
            {error}
          </p>
        )}
      </div>

      <Dialog open={confirming} onClose={() => setConfirming(false)} label="Discard this draft?">
        <Modal
          title="Discard this draft?"
          description="It is deleted from this computer and cannot be brought back. Mail you already sent is not affected."
          onClose={() => setConfirming(false)}
          footer={
            <>
              <Button onClick={() => setConfirming(false)}>Keep writing</Button>
              <Button variant="ink" onClick={() => void discard()}>
                Discard
              </Button>
            </>
          }
        >
          <p className="text-ui text-text-2">
            {files.length > 0
              ? `Its ${files.length === 1 ? 'file is' : `${files.length} files are`} removed too.`
              : 'Nothing else is affected.'}
          </p>
        </Modal>
      </Dialog>

      <Dialog open={picking} onClose={() => setPicking(false)} label="Send at">
        <Modal
          title="Send at"
          description="ZSortMail sends it at this time, as long as it is running, even with the window closed."
          onClose={() => setPicking(false)}
          onSubmit={(e) => {
            e.preventDefault();
            const at = new Date(pickedTime).getTime();
            if (Number.isFinite(at) && at > now()) {
              setPicking(false);
              void schedule(at);
            }
          }}
          footer={
            <>
              <Button onClick={() => setPicking(false)}>Cancel</Button>
              <Button type="submit" variant="primary">
                Schedule
              </Button>
            </>
          }
        >
          <input
            type="datetime-local"
            aria-label="Date and time"
            value={pickedTime}
            min={localInputValue(now() + 60_000)}
            onChange={(e) => setPickedTime(e.target.value)}
          />
        </Modal>
      </Dialog>
    </section>
  );
}
