import { Clock, FastForward, PencilSimple, Plus, Trash } from '@phosphor-icons/react';
import { useState } from 'react';
import { addressLabel } from '@/lib/addresses';
import { api } from '@/lib/api';
import type { Routine } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { openDraft } from '@/lib/composer';
import { formatListTime, plural } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { qk, queryClient, useRoutines } from '@/lib/queries';
import { describeRecurrence } from '@/lib/routines';
import { toast, toastError } from '@/lib/toasts';
import { Button, IconButton } from '@/ui/controls';
import { Toggle } from '@/ui/Toggle';
import { RoutineDialog } from './RoutineDialog';

const EXAMPLES: { name: string; subject: string; body: string; kind: 'weekly' | 'monthly' }[] = [
  {
    name: 'Weekly report',
    subject: 'Weekly report, {{date}}',
    body: 'Hello,\n\nHere is the report for the week. Last one went out {{last_sent}}.',
    kind: 'weekly',
  },
  {
    name: 'Rent',
    subject: 'Rent for {{month}}',
    body: 'Hello,\n\nThe rent for {{month}} {{year}} is on its way.',
    kind: 'monthly',
  },
];

function nextLine(r: Routine): string {
  if (!r.enabled) return 'Paused';
  if (r.recurrence.kind === 'manual') return 'Runs only when you ask';
  if (r.next_run_at === null) return 'Not scheduled';
  const at = new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(r.next_run_at);
  return r.skip_next ? `Next run, ${at}, will be skipped` : `Next: ${at}`;
}

const RESULT = { prepared: 'Draft prepared', sent: 'Sent by itself', skipped: 'Skipped' } as const;

export function RoutinesScreen() {
  const t = useT();
  const { data: routines = [], isSuccess } = useRoutines();
  const [dialog, setDialog] = useState<{ editing?: Routine } | null>(null);

  function setEnabled(r: Routine, enabled: boolean) {
    queryClient.setQueryData<Routine[]>(qk.routines, (old) =>
      old?.map((x) => (x.id === r.id ? { ...x, enabled } : x)),
    );
    api
      .saveRoutine({
        id: r.id,
        name: r.name,
        enabled,
        to: r.to,
        cc: r.cc,
        subject: r.subject,
        body: r.body,
        recurrence: r.recurrence,
        mode: r.mode,
      })
      .catch((e) => {
        toastError(e);
        void queryClient.invalidateQueries({ queryKey: qk.routines });
      });
  }

  async function writeNow(r: Routine) {
    try {
      const out = await api.runRoutine(r.id);
      if (out.draft_id !== null) openDraft(out.draft_id);
    } catch (e) {
      toastError(e);
    }
  }

  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Routines">
      <div className="mx-auto max-w-[820px]">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-title font-semibold tracking-[-0.01em]">{t('routines.title')}</h1>
            <p className="mt-1 max-w-[56ch] text-ui text-text-2">
              Emails that come round again. At the time you pick, the email is written for you with
              today's date filled in, and left as a draft to look over. Routines run while ZSortMail
              is running, even with the window closed.
            </p>
          </div>
          <Button variant="primary" className="shrink-0" onClick={() => setDialog({})}>
            <Plus size={16} weight="bold" />
            New routine
          </Button>
        </div>

        {isSuccess && routines.length === 0 && (
          <div className="mt-12 max-w-[56ch]">
            <h2 className="text-name font-semibold">No routines yet</h2>
            <p className="mt-1 text-ui text-text-2">
              Something you write every week or every month? Write it once.
            </p>
            <ul className="mt-3">
              {EXAMPLES.map((e) => (
                <li key={e.name} className="border-b border-rule">
                  <button
                    type="button"
                    onClick={() => setDialog({})}
                    className="flex w-full items-baseline justify-between gap-4 py-2.5 text-left hover:bg-hover"
                  >
                    <span className="text-ui font-medium">{e.name}</span>
                    <span className="truncate text-ui text-text-2">{e.subject}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}

        <ol className="mt-6 border-t border-rule" aria-label="Routines in order of the next run">
          {routines.map((r) => (
            <li key={r.id} className={cn('border-b border-rule py-4', !r.enabled && 'opacity-60')}>
              <div className="flex items-start gap-4">
                <Toggle
                  label={`Run ${r.name}`}
                  className="mt-0.5"
                  checked={r.enabled}
                  onChange={(on) => setEnabled(r, on)}
                />
                <div className="min-w-0 flex-1">
                  <h3 dir="auto" className="text-body font-semibold">
                    {r.name}
                  </h3>
                  <p className="mt-0.5 text-ui text-text-2">
                    {describeRecurrence(r.recurrence)}.{' '}
                    {r.mode === 'send' ? 'Sends by itself' : 'Prepares a draft for you'}
                    {r.to.length > 0 && (
                      <>
                        {' '}
                        to{' '}
                        <span dir="auto" className="text-text">
                          {addressLabel(r.to[0] as (typeof r.to)[number])}
                          {r.to.length > 1 && ` and ${plural(r.to.length - 1, 'other')}`}
                        </span>
                      </>
                    )}
                    .
                  </p>
                  <p className="mt-0.5 flex flex-wrap items-center gap-x-3 text-meta text-text-2">
                    <span className="inline-flex items-center gap-1">
                      <Clock size={12} />
                      {nextLine(r)}
                    </span>
                    {r.last_run_at !== null && r.last_result && (
                      <span>
                        Last: {RESULT[r.last_result].toLowerCase()},{' '}
                        {formatListTime(r.last_run_at, now())}
                      </span>
                    )}
                    {r.streak >= 2 && <span>On time {r.streak} runs in a row</span>}
                  </p>
                </div>
                <div className="flex shrink-0 items-center">
                  <Button
                    onClick={() => void writeNow(r)}
                    title="Prepare this email now, as a draft"
                  >
                    Write now
                  </Button>
                  {r.recurrence.kind !== 'manual' && (
                    <IconButton
                      label={r.skip_next ? 'Do not skip the next run' : 'Skip the next run'}
                      aria-pressed={r.skip_next}
                      disabled={!r.enabled}
                      onClick={() =>
                        void api
                          .skipRoutine(r.id, !r.skip_next)
                          .then(
                            () => toast(r.skip_next ? 'Next run back on.' : 'Next run skipped.'),
                            toastError,
                          )
                      }
                    >
                      <FastForward size={17} weight={r.skip_next ? 'fill' : 'regular'} />
                    </IconButton>
                  )}
                  <IconButton label="Edit routine" onClick={() => setDialog({ editing: r })}>
                    <PencilSimple size={17} />
                  </IconButton>
                  <IconButton
                    label="Delete routine"
                    onClick={() => void api.deleteRoutine(r.id).catch(toastError)}
                  >
                    <Trash size={17} />
                  </IconButton>
                </div>
              </div>
            </li>
          ))}
        </ol>
      </div>
      {dialog && <RoutineDialog open onClose={() => setDialog(null)} editing={dialog.editing} />}
    </section>
  );
}
