import { useRef, useState } from 'react';
import { type CoreEvent, commands, events } from '@/lib/bindings';
import { cn } from '@/lib/cn';
import { isTauri, zsmUrl } from '@/lib/tauri';
import { Button } from '@/ui/controls';
import { type BenchResult, runFoldBench, runScrollBench, runStrandBench } from './benches';
import { runCapabilityChecks } from './capabilities';
import { measureRefreshMs } from './stats';

interface Row {
  label: string;
  value: string;
  ok?: boolean;
}

interface Section {
  id: string;
  title: string;
  rows: Row[];
}

const ORDER = ['env', 'caps', 'ipc', 'attacks', 'scroll', 'fold', 'strand'] as const;
const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
let autorunStarted = false;

const KNOWN_CSP_GAPS = new Set(['meta-refresh']);

const ms = (n: number) => `${n.toFixed(1)} ms`;

function benchRows(result: BenchResult, refreshMs: number): Row[] {
  const { stats } = result;
  return [
    { label: 'frames', value: `${stats.frames} in ${(stats.durationMs / 1000).toFixed(1)} s` },
    { label: 'average rate', value: `${stats.fps.toFixed(1)} fps` },
    {
      label: 'frame time p50 / p95 / p99',
      value: `${ms(stats.p50Ms)} / ${ms(stats.p95Ms)} / ${ms(stats.p99Ms)}`,
    },
    { label: 'slowest frame', value: ms(stats.maxMs) },
    { label: 'hitches (over 1.5 intervals)', value: String(stats.hitches) },
    ...Object.entries(result.extra).map(([label, value]) => ({ label, value: String(value) })),
    {
      label: `smooth (p95 within 1.5 x ${ms(refreshMs)}, no frame over 4 x)`,
      value: result.smooth ? 'pass' : 'FAIL',
      ok: result.smooth,
    },
  ];
}

export function SpikesScreen() {
  const [sections, setSections] = useState<Section[]>([]);
  const [running, setRunning] = useState(false);
  const benchHost = useRef<HTMLDivElement>(null);
  const frameHost = useRef<HTMLDivElement>(null);

  function put(section: Section) {
    setSections((prev) =>
      [...prev.filter((s) => s.id !== section.id), section].sort(
        (a, b) =>
          ORDER.indexOf(a.id as (typeof ORDER)[number]) -
          ORDER.indexOf(b.id as (typeof ORDER)[number]),
      ),
    );
  }

  function report(name: string, data: unknown) {
    const text = JSON.stringify(data);
    console.info('SPIKE', name, text);
    if (isTauri()) void commands.spikeReport(name, text).catch(() => {});
  }

  async function step(id: (typeof ORDER)[number], title: string, run: () => Promise<Row[]>) {
    try {
      const rows = await run();
      put({ id, title, rows });
      report(id, rows);
    } catch (error) {
      const rows = [{ label: 'error', value: String(error), ok: false }];
      put({ id, title, rows });
      report(id, rows);
    }
  }

  async function runIpc(): Promise<Row[]> {
    if (!isTauri()) return [{ label: 'skipped', value: 'not running in the desktop window' }];
    const info = await commands.appState();
    const rows: Row[] = [
      {
        label: 'Rust core linked',
        value: `${info.name} ${info.version}, ${info.os}/${info.arch}, schema v${info.schema_version}`,
        ok: info.schema_version > 0,
      },
    ];

    const count = 300;
    const t0 = performance.now();
    for (let i = 0; i < count; i++) await commands.ping(Math.round(Date.now() * 1000));
    const sequential = (performance.now() - t0) / count;
    const t1 = performance.now();
    await Promise.all(
      Array.from({ length: count }, () => commands.ping(Math.round(Date.now() * 1000))),
    );
    const parallel = (performance.now() - t1) / count;
    rows.push({
      label: `ping, ${count} one at a time`,
      value: `${sequential.toFixed(3)} ms per call`,
      ok: sequential < 2,
    });
    rows.push({
      label: `ping, ${count} at once`,
      value: `${parallel.toFixed(3)} ms per call`,
      ok: parallel < 2,
    });

    let received: CoreEvent | null = null;
    const unlisten = await events.zsmEvent.listen((e) => {
      received = e.payload;
    });
    await commands.debugEmit('people');
    await sleep(300);
    unlisten();
    rows.push({
      label: 'typed event round trip',
      value: received ? JSON.stringify(received) : 'not received',
      ok: received !== null,
    });

    const loaded = await new Promise<boolean>((resolve) => {
      const frame = document.createElement('iframe');
      frame.setAttribute('sandbox', '');
      frame.style.cssText = 'position:absolute;width:1px;height:1px;opacity:0';
      frame.onload = () => resolve(true);
      frame.onerror = () => resolve(false);
      frameHost.current?.append(frame);
      frame.src = zsmUrl('hello');
      setTimeout(() => resolve(false), 3000);
    });
    rows.push({
      label: `zsm:// protocol (${zsmUrl('hello')})`,
      value: loaded ? 'loaded in sandboxed iframe' : 'did not load',
      ok: loaded,
    });
    return rows;
  }

  async function runAttacks(): Promise<Row[]> {
    if (!isTauri())
      return [{ label: 'skipped', value: 'needs the desktop window and its protocol' }];
    const names = await commands.spikeAttacks();
    const ranScripts = new Set<string>();
    const onMessage = (e: MessageEvent) => {
      if (typeof e.data === 'string' && e.data.startsWith('ran:')) ranScripts.add(e.data.slice(4));
    };
    window.addEventListener('message', onMessage);

    const modes = [
      {
        id: 'policy',
        label: 'production policy: sandbox, no scripts, CSP',
        sandbox: '',
        csp: true,
      },
      {
        id: 'csp-only',
        label: 'CSP alone: scripts allowed, CSP on',
        sandbox: 'allow-scripts',
        csp: true,
      },
      {
        id: 'control',
        label: 'control: scripts allowed, CSP removed',
        sandbox: 'allow-scripts',
        csp: false,
      },
    ] as const;

    const rows: Row[] = [{ label: 'attack pages', value: `${names.length} (${names.join(', ')})` }];
    for (const mode of modes) {
      let hits = 0;
      const hitBy: string[] = [];
      const ran = new Set<string>();
      for (const [i, name] of names.entries()) {
        await commands.spikeBeaconReset();
        ranScripts.clear();
        const frame = document.createElement('iframe');
        frame.setAttribute('sandbox', mode.sandbox);
        frame.style.cssText =
          'position:absolute;width:1px;height:1px;opacity:0;pointer-events:none';
        frame.src = zsmUrl(`spike/attack/${i}${mode.csp ? '' : '?csp=0'}`);
        frameHost.current?.append(frame);
        await sleep(650);
        const beacon = await commands.spikeBeacon();
        if (beacon.hits > 0) {
          hits += beacon.hits;
          hitBy.push(name);
        }
        for (const r of ranScripts) ran.add(r);
        frame.remove();
      }
      const onlyKnownGaps = hitBy.every((name) => KNOWN_CSP_GAPS.has(name));
      const pass =
        mode.id === 'control'
          ? hits > 0
          : mode.id === 'csp-only'
            ? ran.size === 0 && onlyKnownGaps
            : hits === 0 && ran.size === 0;
      const note =
        mode.id === 'csp-only' && hitBy.length > 0 && onlyKnownGaps
          ? ' (known gap: CSP does not govern navigation; the sanitizer strips <meta>)'
          : '';
      rows.push({
        label: mode.label,
        value:
          mode.id === 'control'
            ? `${hits} beacon hits, ${ran.size} scripts ran (${hitBy.join(', ') || 'none'}): ${pass ? 'beacon works' : 'INCONCLUSIVE, beacon saw nothing'}`
            : `${hits} beacon hits, ${ran.size} scripts ran${hitBy.length ? ` (${hitBy.join(', ')})` : ''}${note}`,
        ok: pass,
      });
    }
    window.removeEventListener('message', onMessage);
    return rows;
  }

  async function runAll() {
    setRunning(true);
    const refreshMs = await measureRefreshMs();

    await step('env', 'Environment', async () => {
      await Promise.all([
        document.fonts.load('600 15px "Bricolage Grotesque Variable"'),
        document.fonts.load('16px "Literata Variable"'),
      ]);
      const fonts = [
        ...new Set([...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family)),
      ];
      const intl = new Intl.DateTimeFormat().resolvedOptions();
      return [
        { label: 'webview', value: navigator.userAgent },
        {
          label: 'display',
          value: `${screen.width} x ${screen.height} at ${window.devicePixelRatio}x, refresh about ${(1000 / refreshMs).toFixed(0)} Hz (${ms(refreshMs)})`,
        },
        {
          label: 'colour scheme',
          value: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
        },
        {
          label: 'reduced motion',
          value: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'on' : 'off',
        },
        {
          label: 'locale and clock',
          value: `${navigator.language}, ${intl.locale}, hour cycle ${intl.hourCycle ?? 'unknown'}`,
        },
        { label: 'logical cores', value: String(navigator.hardwareConcurrency) },
        {
          label: 'fonts loaded',
          value: fonts.join(', ') || 'none',
          ok:
            fonts.some((f) => f.startsWith('Bricolage')) &&
            fonts.some((f) => f.startsWith('Literata')),
        },
      ];
    });

    await step('caps', 'Web platform features the design uses', async () =>
      runCapabilityChecks().map((c) => ({
        label: c.name,
        value: `${c.ok ? 'supported' : 'MISSING'}, for ${c.use}`,
        ok: c.ok || c.use.startsWith('not used'),
      })),
    );

    await step('ipc', 'IPC, channel, events and the zsm:// protocol', runIpc);
    await step('attacks', 'Hostile mail: sandbox and CSP', runAttacks);

    const host = () => {
      if (!benchHost.current) throw new Error('bench host missing');
      const el = document.createElement('div');
      benchHost.current.replaceChildren(el);
      return el;
    };
    await step('scroll', 'People list: 5,000 people, scrolled at 3,000 px/s', async () =>
      benchRows(await runScrollBench(host(), refreshMs), refreshMs),
    );
    await step('fold', 'Fold between panes, played continuously', async () =>
      benchRows(await runFoldBench(host(), refreshMs), refreshMs),
    );
    await step('strand', 'Open the 300-message conversation', async () =>
      benchRows(await runStrandBench(host(), refreshMs), refreshMs),
    );

    report('done', { at: new Date().toISOString() });
    setRunning(false);
  }

  const autorun = new URLSearchParams(window.location.search).has('autorun');
  const trigger = (node: HTMLDivElement | null) => {
    if (node && autorun && !autorunStarted) {
      autorunStarted = true;
      void runAll();
    }
  };

  return (
    <div ref={trigger} className="scroll-y h-full bg-pane px-10 py-8" data-testid="spikes">
      <div className="mx-auto max-w-[980px]">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h1 className="text-title font-semibold tracking-[-0.01em]">Risk spikes</h1>
            <p className="mt-1 max-w-[64ch] text-ui text-text-2">
              Development only. Measures the plan’s risky assumptions in the webview that is running
              now. Add <code>?autorun</code> to the address to run on load.
            </p>
          </div>
          <Button variant="ink" disabled={running} onClick={() => void runAll()}>
            {running ? 'Running' : 'Run all'}
          </Button>
        </div>

        <div className="mt-8 space-y-8">
          {sections.map((section) => (
            <section key={section.id}>
              <h2 className="text-name font-semibold">{section.title}</h2>
              <dl className="mt-2 divide-y divide-rule border-y border-rule">
                {section.rows.map((row) => (
                  <div
                    key={row.label}
                    className="grid grid-cols-[minmax(180px,1fr)_2fr] gap-4 py-1.5"
                  >
                    <dt className="text-ui text-text-2">{row.label}</dt>
                    <dd
                      className={cn(
                        'tabular text-ui break-words',
                        row.ok === true && 'font-medium',
                        row.ok === false && 'font-semibold text-red',
                      )}
                    >
                      {row.value}
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>

        <div ref={benchHost} className="mt-8" />
        <div ref={frameHost} aria-hidden="true" />
      </div>
    </div>
  );
}
