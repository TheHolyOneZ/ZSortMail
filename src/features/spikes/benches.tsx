import { QueryClientProvider } from '@tanstack/react-query';
import { useVirtualizer } from '@tanstack/react-virtual';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import { api } from '@/lib/api';
import type { PersonSummary } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { queryClient } from '@/lib/queries';
import { FoldStack } from '@/motion/choreographies/fold';
import { MotionTierOverride } from '@/motion/MotionProvider';
import { Stamp } from '@/ui/Stamp';
import { PersonRow } from '../people/PeoplePane';
import { StrandView } from '../strand/StrandView';
import { type FrameStats, nextFrame, sampleFrames, summarize, worstFrames } from './stats';

function syntheticPeople(count: number): PersonSummary[] {
  const first = [
    'Aaliyah',
    'Bruno',
    'Camille',
    'Dmitri',
    'Elena',
    'Farid',
    'Greta',
    'Hugo',
    'Isabel',
    'Jonas',
  ];
  const last = [
    'Abara',
    'Brandt',
    'Castillo',
    'Dvorak',
    'Eriksen',
    'Fontaine',
    'Gallagher',
    'Haddad',
  ];
  const base = now();
  return Array.from({ length: count }, (_, i) => ({
    id: i + 1,
    name: `${first[i % first.length]} ${last[(i * 7) % last.length]}`,
    kind: 'person',
    status: 'known',
    ink: i % 10,
    title: i % 3 === 0 ? 'Designer' : null,
    last_subject: `Subject number ${i}`,
    last_at: base - i * 3_600_000,
    unread: i % 11 === 0 ? 1 : 0,
    pinned: false,
    needs_reply: i % 5 === 0,
    waiting_on_them: i % 7 === 0,
    has_files: i % 4 === 0,
    photo_url: null,
    vip: false,
  }));
}

function VirtualBench({ people }: { people: PersonSummary[] }) {
  const ref = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: people.length,
    getScrollElement: () => ref.current,
    estimateSize: () => 60,
    overscan: 8,
  });
  const nowTs = now();
  return (
    <div
      ref={ref}
      data-bench-scroller
      className="scroll-y"
      style={{ height: 640, width: 320, background: 'var(--pane)' }}
    >
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((item) => {
          const person = people[item.index];
          if (!person) return null;
          return (
            <PersonRow
              key={person.id}
              person={person}
              selected={item.index === 3}
              nowTs={nowTs}
              onOpen={() => {}}
              style={{ height: item.size, transform: `translateY(${item.start}px)` }}
            />
          );
        })}
      </div>
    </div>
  );
}

function FakePane({ title }: { title: string }) {
  return (
    <div style={{ padding: 16 }}>
      <p className="text-name font-semibold">{title}</p>
      {Array.from({ length: 24 }, (_, i) => i).map((i) => (
        <div key={i} className="flex items-center gap-3 border-b border-rule py-2">
          <Stamp name={`Person ${i}`} ink={i} size={36} />
          <div>
            <p className="text-body font-medium">Conversation number {i}</p>
            <p className="text-ui text-text-2">A line of preview text for this row</p>
          </div>
        </div>
      ))}
    </div>
  );
}

function FoldBench() {
  const [level, setLevel] = useState(0);
  useEffect(() => {
    const sequence = [1, 2, 1, 0];
    let i = 0;
    const id = window.setInterval(() => {
      setLevel(sequence[i % sequence.length] ?? 0);
      i += 1;
    }, 450);
    return () => window.clearInterval(id);
  }, []);
  return (
    <div style={{ height: 480, width: 640, background: 'var(--pane)' }}>
      <FoldStack
        level={level}
        panes={[
          <FakePane key="a" title="People" />,
          <FakePane key="b" title="Conversations" />,
          <FakePane key="c" title="Conversation" />,
        ]}
      />
    </div>
  );
}

export interface BenchResult {
  stats: FrameStats;
  smooth: boolean;
  extra: Record<string, number | string>;
}

const BUDGET_P95_INTERVALS = 1.5;
const BUDGET_MAX_INTERVALS = 4;

function judge(stats: FrameStats, refreshMs: number): boolean {
  return (
    stats.p95Ms <= refreshMs * BUDGET_P95_INTERVALS &&
    stats.maxMs <= refreshMs * BUDGET_MAX_INTERVALS
  );
}

function mount(root: Root, node: ReactNode): void {
  flushSync(() => root.render(node));
}

async function waitFor(
  host: HTMLElement,
  selector: string,
  timeoutMs = 5000,
): Promise<HTMLElement> {
  const deadline = performance.now() + timeoutMs;
  for (;;) {
    const el = host.querySelector<HTMLElement>(selector);
    if (el) return el;
    if (performance.now() > deadline) {
      throw new Error(`"${selector}" did not appear within ${timeoutMs} ms`);
    }
    await nextFrame();
  }
}

export async function runScrollBench(host: HTMLElement, refreshMs: number): Promise<BenchResult> {
  const people = syntheticPeople(5000);
  const root = createRoot(host);
  mount(root, <VirtualBench people={people} />);
  const scroller = await waitFor(host, '[data-bench-scroller]');
  await nextFrame();
  await nextFrame();

  const pxPerSecond = 3000;
  const travel = Math.max(1, scroller.scrollHeight - scroller.clientHeight);
  const deltas = await sampleFrames(4000, (dt) => {
    scroller.scrollTop = (scroller.scrollTop + (pxPerSecond * dt) / 1000) % travel;
  });
  root.unmount();
  const stats = summarize(deltas, refreshMs);
  return {
    stats,
    smooth: judge(stats, refreshMs),
    extra: {
      people: people.length,
      pxPerSecond,
      'worst frames (ms at time)': worstFrames(deltas)
        .map((w) => `${w.frameMs.toFixed(0)}@${(w.atMs / 1000).toFixed(2)}s`)
        .join(', '),
    },
  };
}

export async function runFoldBench(host: HTMLElement, refreshMs: number): Promise<BenchResult> {
  const root = createRoot(host);
  mount(
    root,
    <MotionTierOverride tier="full">
      <FoldBench />
    </MotionTierOverride>,
  );
  await nextFrame();
  await nextFrame();
  const deltas = await sampleFrames(4500);
  root.unmount();
  const stats = summarize(deltas, refreshMs);
  return { stats, smooth: judge(stats, refreshMs), extra: { transitions: 10 } };
}

async function openConversation(
  host: HTMLElement,
  conversationId: number,
  personId: number,
  knots: number,
): Promise<{ root: Root; openMs: number }> {
  const started = performance.now();
  const root = createRoot(host);
  mount(
    root,
    <QueryClientProvider client={queryClient}>
      <MotionTierOverride tier="full">
        <div style={{ height: 640, width: 900 }}>
          <StrandView conversationId={conversationId} personId={personId} />
        </div>
      </MotionTierOverride>
    </QueryClientProvider>,
  );
  const deadline = started + 10_000;
  while (host.querySelectorAll('[data-knot]').length < knots) {
    if (performance.now() > deadline) throw new Error('strand did not render in time');
    await nextFrame();
  }
  await nextFrame();
  await nextFrame();
  return { root, openMs: performance.now() - started };
}

export async function runStrandBench(host: HTMLElement, refreshMs: number): Promise<BenchResult> {
  const maya = (await api.listPeople('all', 'Okafor'))[0];
  const dan = (await api.listPeople('all', 'Whitaker'))[0];
  if (!maya || !dan) throw new Error('fixture people not found');
  const typical = (await api.listConversations(maya.id)).find((c) => c.message_count === 14);
  const long = (await api.listConversations(dan.id)).find((c) => c.message_count >= 300);
  if (!typical || !long) throw new Error('fixture conversations not found');

  const cold = await openConversation(host, typical.id, maya.id, typical.message_count);
  cold.root.unmount();
  const warm = await openConversation(host, typical.id, maya.id, typical.message_count);
  warm.root.unmount();

  const extreme = await openConversation(host, long.id, dan.id, long.message_count);
  const deltas = await sampleFrames(900);
  extreme.root.unmount();

  const stats = summarize(deltas, refreshMs);
  return {
    stats,
    smooth: judge(stats, refreshMs),
    extra: {
      [`open ${typical.message_count} messages, first time (ms)`]: Math.round(cold.openMs),
      [`open ${typical.message_count} messages, warm (ms)`]: Math.round(warm.openMs),
      [`open ${long.message_count} messages (ms)`]: Math.round(extreme.openMs),
    },
  };
}
