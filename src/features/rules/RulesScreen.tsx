import { ArrowDown, ArrowUp, PencilSimple, Play, Plus, Trash } from '@phosphor-icons/react';
import { useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { Rule } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { formatListTime, plural } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { qk, queryClient, useRules } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { Button, IconButton } from '@/ui/controls';
import { Toggle } from '@/ui/Toggle';
import { describeActions, RuleDialog } from './RuleDialog';

const EXAMPLES = [
  { query: 'category:newsletter', hint: 'Keep newsletters out of the inbox' },
  { query: 'from:noreply subject:receipt', hint: 'File receipts away, read' },
  { query: 'from:notifications is:unread', hint: 'Quiet the automated noise' },
];

export function RulesScreen() {
  const t = useT();
  const { data: rules = [], isSuccess } = useRules();
  const [dialog, setDialog] = useState<{ editing?: Rule; query?: string } | null>(null);

  function setEnabled(rule: Rule, enabled: boolean) {
    queryClient.setQueryData<Rule[]>(qk.rules, (old) =>
      old?.map((r) => (r.id === rule.id ? { ...r, enabled } : r)),
    );
    api
      .saveRule({
        id: rule.id,
        name: rule.name,
        enabled,
        query: rule.query,
        actions: rule.actions,
        stop: rule.stop,
      })
      .catch((e) => {
        toastError(e);
        void queryClient.invalidateQueries({ queryKey: qk.rules });
      });
  }

  function move(index: number, by: number) {
    const ids = rules.map((r) => r.id);
    const [moved] = ids.splice(index, 1);
    if (moved === undefined) return;
    ids.splice(index + by, 0, moved);
    void api.orderRules(ids).catch(toastError);
  }

  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Rules">
      <div className="mx-auto max-w-[820px]">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-title font-semibold tracking-[-0.01em]">{t('rules.title')}</h1>
            <p className="mt-1 max-w-[56ch] text-ui text-text-2">
              When new mail matches a search, do something with it. Rules run in this order. A
              person you always allow is never touched by them.
            </p>
          </div>
          <Button variant="primary" className="shrink-0" onClick={() => setDialog({})}>
            <Plus size={16} weight="bold" />
            New rule
          </Button>
        </div>

        {isSuccess && rules.length === 0 && (
          <div className="mt-12 max-w-[56ch]">
            <h2 className="text-name font-semibold">No rules yet</h2>
            <p className="mt-1 text-ui text-text-2">Start from an idea, or write your own.</p>
            <ul className="mt-3">
              {EXAMPLES.map((e) => (
                <li key={e.query} className="border-b border-rule">
                  <button
                    type="button"
                    onClick={() => setDialog({ query: e.query })}
                    className="flex w-full items-baseline justify-between gap-4 py-2.5 text-left hover:bg-hover"
                  >
                    <span className="text-ui font-medium">{e.query}</span>
                    <span className="text-ui text-text-2">{e.hint}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}

        <ol className="mt-6 border-t border-rule" aria-label="Rules in order">
          {rules.map((rule, i) => (
            <li
              key={rule.id}
              className={cn('border-b border-rule py-4', !rule.enabled && 'opacity-60')}
            >
              <div className="flex items-start gap-4">
                <Toggle
                  label={`Run ${rule.name}`}
                  className="mt-0.5"
                  checked={rule.enabled}
                  onChange={(on) => setEnabled(rule, on)}
                />
                <div className="min-w-0 flex-1">
                  <h3 dir="auto" className="text-body font-semibold">
                    {rule.name}
                  </h3>
                  <p className="mt-0.5 text-ui text-text-2">
                    When{' '}
                    <span dir="auto" className="font-medium text-text">
                      {rule.query}
                    </span>
                    , {describeActions(rule.actions)}
                    {rule.stop ? ', then stop' : ''}.
                  </p>
                  <p className="mt-0.5 text-meta text-text-2">
                    {rule.hit_count === 0
                      ? 'Has not matched anything yet'
                      : `Caught ${plural(rule.hit_count, 'message')}${rule.last_hit_at ? `, last ${formatListTime(rule.last_hit_at, now())}` : ''}`}
                  </p>
                </div>
                <div className="flex shrink-0 items-center">
                  <Button
                    onClick={() => void act(() => api.applyRule(rule.id))}
                    title="Run this rule on the mail you already have"
                  >
                    <Play size={14} weight="fill" />
                    Apply to existing
                  </Button>
                  <IconButton label="Move up" disabled={i === 0} onClick={() => move(i, -1)}>
                    <ArrowUp size={17} />
                  </IconButton>
                  <IconButton
                    label="Move down"
                    disabled={i === rules.length - 1}
                    onClick={() => move(i, 1)}
                  >
                    <ArrowDown size={17} />
                  </IconButton>
                  <IconButton label="Edit rule" onClick={() => setDialog({ editing: rule })}>
                    <PencilSimple size={17} />
                  </IconButton>
                  <IconButton
                    label="Delete rule"
                    onClick={() => void api.deleteRule(rule.id).catch(toastError)}
                  >
                    <Trash size={17} />
                  </IconButton>
                </div>
              </div>
            </li>
          ))}
        </ol>
      </div>
      {dialog && <RuleDialog open onClose={() => setDialog(null)} editing={dialog.editing} />}
    </section>
  );
}
