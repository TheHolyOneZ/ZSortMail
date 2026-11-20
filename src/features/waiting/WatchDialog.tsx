import { useState } from 'react';
import { api } from '@/lib/api';
import type { WatchView } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { formatListTime, plural } from '@/lib/format';
import { usePreview } from '@/lib/queries';
import { toast, toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Choice, Field, Modal, Section } from '@/ui/Modal';

function dayAfter(days: number): string {
  const d = new Date(now() + days * 86_400_000);
  return new Intl.DateTimeFormat('en-CA').format(d);
}

export function WatchDialog({
  open,
  onClose,
  initialQuery = '',
  editing,
}: {
  open: boolean;
  onClose: () => void;
  initialQuery?: string;
  editing?: WatchView;
}) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      label={editing ? 'Change what you are waiting for' : 'Wait for an email'}
      className="[--dialog-w:560px]"
    >
      <WatchForm
        onClose={onClose}
        initialQuery={editing?.query ?? initialQuery}
        editing={editing}
      />
    </Dialog>
  );
}

function WatchForm({
  onClose,
  initialQuery,
  editing,
}: {
  onClose: () => void;
  initialQuery: string;
  editing?: WatchView;
}) {
  const [query, setQuery] = useState(initialQuery);
  const [keep, setKeep] = useState(editing?.mode === 'keep');
  const [urgent, setUrgent] = useState(editing?.priority === 'urgent');
  const [existing, setExisting] = useState(false);
  const [ends, setEnds] = useState(editing?.expires_at ? dayAfter(0) : '');
  const [saving, setSaving] = useState(false);
  const preview = usePreview(query);
  const found = preview.data;

  async function save() {
    setSaving(true);
    try {
      const expires = ends ? new Date(`${ends}T23:59:59`).getTime() : null;
      const saved = await api.saveWatch({
        id: editing?.id ?? null,
        name: '',
        query: query.trim(),
        mode: keep ? 'keep' : 'once',
        priority: urgent ? 'urgent' : 'normal',
        expires_at: expires,
        include_existing: existing && !editing,
      });
      toast(
        saved.counted > 0
          ? `Waiting for “${query.trim()}”. ${plural(saved.counted, 'message')} already here counted.`
          : `Waiting for “${query.trim()}”.`,
      );
      onClose();
    } catch (e) {
      toastError(e);
      setSaving(false);
    }
  }

  return (
    <Modal
      title={editing ? 'Change what you are waiting for' : 'Wait for an email'}
      description="Say what the email will mention, or use search operators. You are told the moment it arrives."
      onClose={onClose}
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={saving || !query.trim()}>
            {editing ? 'Save' : 'Start watching'}
          </Button>
        </>
      }
    >
      <Field label="What are you waiting for?">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="xbox 394, or from:dhl has:attachment"
          // biome-ignore lint/a11y/noAutofocus: the dialog exists for this one field.
          autoFocus
          autoComplete="off"
          spellCheck={false}
          dir="auto"
        />
      </Field>

      <div className="mt-2 min-h-[44px] text-ui text-text-2" aria-live="polite">
        {query.trim() === '' ? null : found === undefined ? (
          'Checking'
        ) : found.count === 0 ? (
          'Nothing here matches yet. A watch reacts to mail that arrives from now on.'
        ) : (
          <>
            {plural(found.count, 'message')} here already match.
            <ul className="mt-1">
              {found.samples.slice(0, 3).map((s) => (
                <li key={s.message_id} dir="auto" className="truncate">
                  {s.from.is_me ? 'You' : s.from.name}: {s.subject},{' '}
                  {formatListTime(s.sent_at, now())}
                </li>
              ))}
            </ul>
          </>
        )}
      </div>

      <Section title="How to watch">
        <div className="space-y-2">
          <Choice
            checked={keep}
            onChange={setKeep}
            title="Keep watching after the first match"
            hint="Useful for a shop or a person you hear from often."
          />
          <Choice
            checked={urgent}
            onChange={setUrgent}
            title="Urgent"
            hint="Notify even in quiet hours and Focus mode, and keep it marked until opened."
          />
          {!editing && (
            <Choice
              checked={existing}
              onChange={setExisting}
              title="Also count mail that is already here"
              hint="Otherwise only new mail counts."
            />
          )}
        </div>
      </Section>

      <Section title="Until when">
        <Field label="Stop watching on" hint="Leave empty to watch until it arrives.">
          <input
            className="w-52"
            type="date"
            value={ends}
            min={dayAfter(1)}
            onChange={(e) => setEnds(e.target.value)}
          />
        </Field>
      </Section>
    </Modal>
  );
}
