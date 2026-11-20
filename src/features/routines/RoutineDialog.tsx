import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { api } from '@/lib/api';
import type { Address, Recurrence, RecurrenceKind, Routine, RoutineMode } from '@/lib/bindings';
import { cn } from '@/lib/cn';
import { useRoutineVariables } from '@/lib/queries';
import { DAY_NAMES, minutesToTime, timeToMinutes } from '@/lib/routines';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Choice, Field, Modal, Section } from '@/ui/Modal';
import { Select } from '@/ui/Select';
import { RecipientField } from '../compose/RecipientField';

const KINDS: { value: RecurrenceKind; label: string }[] = [
  { value: 'daily', label: 'Every day' },
  { value: 'weekdays', label: 'Every weekday' },
  { value: 'weekly', label: 'Every week' },
  { value: 'monthly', label: 'Every month' },
  { value: 'manual', label: 'Only when I ask' },
];

export function RoutineDialog({
  open,
  onClose,
  editing,
}: {
  open: boolean;
  onClose: () => void;
  editing?: Routine;
}) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      label={editing ? 'Edit routine' : 'New routine'}
      className="[--dialog-w:640px]"
    >
      <Form onClose={onClose} editing={editing} />
    </Dialog>
  );
}

function usePreviewText(text: string) {
  return useQuery({
    queryKey: ['routinePreview', text],
    queryFn: () => api.previewRoutineText(text),
    enabled: text.includes('{{'),
    placeholderData: keepPreviousData,
  });
}

function Form({ onClose, editing }: { onClose: () => void; editing?: Routine }) {
  const [name, setName] = useState(editing?.name ?? '');
  const [to, setTo] = useState<Address[]>(editing?.to ?? []);
  const [subject, setSubject] = useState(editing?.subject ?? '');
  const [body, setBody] = useState(editing?.body ?? '');
  const [kind, setKind] = useState<RecurrenceKind>(editing?.recurrence.kind ?? 'weekly');
  const [time, setTime] = useState(minutesToTime(editing?.recurrence.minutes ?? 9 * 60));
  const [days, setDays] = useState<number[]>(editing?.recurrence.days ?? [1]);
  const [dom, setDom] = useState(editing?.recurrence.day_of_month ?? 1);
  const [mode, setMode] = useState<RoutineMode>(editing?.mode ?? 'draft');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const subjectField = useRef<HTMLInputElement>(null);
  const bodyField = useRef<HTMLTextAreaElement>(null);
  const lastField = useRef<'subject' | 'body'>('body');
  const { data: variables = [] } = useRoutineVariables();
  const subjectPreview = usePreviewText(subject).data;
  const bodyPreview = usePreviewText(body).data;

  function insert(variable: string) {
    const token = `{{${variable}}}`;
    const into = lastField.current === 'subject' ? subjectField.current : bodyField.current;
    const current = lastField.current === 'subject' ? subject : body;
    const set = lastField.current === 'subject' ? setSubject : setBody;
    const start = into?.selectionStart ?? current.length;
    const end = into?.selectionEnd ?? current.length;
    set(current.slice(0, start) + token + current.slice(end));
    requestAnimationFrame(() => {
      into?.focus();
      into?.setSelectionRange(start + token.length, start + token.length);
    });
  }

  function toggleDay(day: number) {
    setDays((prev) => (prev.includes(day) ? prev.filter((d) => d !== day) : [...prev, day]));
  }

  async function save() {
    setSaving(true);
    setError(null);
    const recurrence: Recurrence = {
      kind,
      minutes: timeToMinutes(time),
      days: kind === 'weekly' ? [...days].sort((a, b) => a - b) : [],
      day_of_month: dom,
    };
    try {
      await api.saveRoutine({
        id: editing?.id ?? null,
        name,
        enabled: editing?.enabled ?? true,
        to,
        cc: editing?.cc ?? [],
        subject,
        body,
        recurrence,
        mode,
      });
      onClose();
    } catch (e) {
      setError(String((e as { message?: string }).message ?? e));
      setSaving(false);
    }
  }

  return (
    <Modal
      title={editing ? 'Edit routine' : 'New routine'}
      description="An email that comes round again. At its time it is written for you, with today's date filled in."
      onClose={onClose}
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
      footer={
        <>
          {error && (
            <p role="alert" className="me-auto text-ui text-red">
              {error}
            </p>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button
            type="submit"
            variant="primary"
            disabled={saving || !subject.trim() || !body.trim()}
          >
            Save routine
          </Button>
        </>
      }
    >
      <Field label="Name">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Weekly report. The subject is used if you leave it empty."
          dir="auto"
          // biome-ignore lint/a11y/noAutofocus: the dialog exists for this form.
          autoFocus={!editing}
        />
      </Field>

      <Section title="What it says">
        <div className="overflow-visible rounded-control bg-hover">
          <RecipientField label="To" value={to} onChange={setTo} tone="desk" />
        </div>
        <Field label="Subject" className="mt-3">
          <input
            ref={subjectField}
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            onFocus={() => {
              lastField.current = 'subject';
            }}
            placeholder="Weekly report, {{date}}"
            dir="auto"
          />
        </Field>
        <Field label="Text" className="mt-3">
          <textarea
            ref={bodyField}
            className="letter"
            value={body}
            rows={6}
            onChange={(e) => setBody(e.target.value)}
            onFocus={() => {
              lastField.current = 'body';
            }}
            placeholder={'Hello,\n\nHere is the report for {{weekday}}.'}
            dir="auto"
          />
        </Field>
        <div className="mt-2 flex flex-wrap items-center gap-1.5">
          <span className="text-meta text-text-2">Insert</span>
          {variables.map((v) => (
            <button
              key={v.name}
              type="button"
              title={`${v.name}: ${v.meaning}`}
              onClick={() => insert(v.name)}
              className="rounded-full border border-rule-strong px-2.5 py-0.5 text-meta text-text-2 transition-colors hover:bg-hover hover:text-text"
            >
              {v.name}
            </button>
          ))}
        </div>
        {(subjectPreview !== undefined || bodyPreview !== undefined) && (
          <p
            className="mt-3 rounded-control bg-blue-wash px-3 py-2 text-ui text-text-2"
            aria-live="polite"
          >
            Written today, it says:{' '}
            <span dir="auto" className="font-medium text-text">
              {subjectPreview ?? subject}
            </span>
            {bodyPreview !== undefined && (
              <span dir="auto" className="mt-1 block whitespace-pre-line">
                {bodyPreview.split('\n').filter(Boolean).slice(0, 2).join('\n')}
              </span>
            )}
          </p>
        )}
      </Section>

      <Section title="When">
        <div className="flex flex-wrap items-center gap-3">
          <Select<RecurrenceKind>
            label="How often"
            className="w-44"
            value={kind}
            onChange={setKind}
            options={KINDS.map((k) => ({ value: k.value, label: k.label }))}
          />
          {kind !== 'manual' && (
            <label className="flex items-center gap-2 text-ui">
              at
              <input
                type="time"
                className="!w-auto"
                value={time}
                onChange={(e) => setTime(e.target.value)}
              />
            </label>
          )}
          {kind === 'monthly' && (
            <div className="flex items-center gap-2 text-ui">
              on day
              <Select<number>
                label="Day of the month"
                className="w-20"
                value={dom}
                onChange={setDom}
                options={Array.from({ length: 31 }, (_, i) => ({
                  value: i + 1,
                  label: String(i + 1),
                }))}
              />
            </div>
          )}
        </div>
        {kind === 'weekly' && (
          <fieldset className="mt-3 flex flex-wrap gap-1.5">
            <legend className="sr-only">Days of the week</legend>
            {DAY_NAMES.map((d, i) => (
              <button
                key={d}
                type="button"
                aria-pressed={days.includes(i + 1)}
                aria-label={d}
                onClick={() => toggleDay(i + 1)}
                className={cn(
                  'h-8 rounded-control border px-3 text-ui transition-colors',
                  days.includes(i + 1)
                    ? 'border-blue bg-blue-wash font-medium text-text'
                    : 'border-rule-strong text-text-2 hover:bg-hover',
                )}
              >
                {d.slice(0, 3)}
              </button>
            ))}
          </fieldset>
        )}
        {kind === 'monthly' && dom > 28 && (
          <p className="mt-2 text-meta text-text-2">
            In months with fewer days it runs on the last day.
          </p>
        )}
      </Section>

      <Section title="What happens">
        <div className="grid gap-2">
          <Choice
            type="radio"
            name="routine-mode"
            checked={mode === 'draft'}
            onChange={() => setMode('draft')}
            title="Prepare a draft and tell me"
            hint="You look it over and send it. Nothing leaves without you."
          />
          <Choice
            type="radio"
            name="routine-mode"
            checked={mode === 'send'}
            onChange={() => setMode('send')}
            title="Send it by itself"
            hint="It goes out at its time. If the computer was off then, it prepares a draft instead."
          />
        </div>
      </Section>
    </Modal>
  );
}
