import { useEffect, useRef, useState } from 'react';
import { DAY, HOUR, MINUTE } from '@/lib/clock';
import { gapLabel } from '@/lib/format';
import { initials } from '@/lib/ink';
import { FoldStack } from '@/motion/choreographies/fold';
import { Postmark } from '@/motion/choreographies/stamp';
import { useMotionGovernor } from '@/motion/MotionProvider';
import { GAP_MAX_PX, gapPx } from '@/motion/strand';
import { Airmail } from '@/ui/Airmail';
import { Button, Chip, Kbd, SearchField, Segmented } from '@/ui/controls';
import { Stamp } from '@/ui/Stamp';

function toHex(css: string): string {
  const ctx = document.createElement('canvas').getContext('2d');
  if (!ctx) return css;
  ctx.fillStyle = '#000';
  ctx.fillStyle = css;
  ctx.fillRect(0, 0, 1, 1);
  const [r = 0, g = 0, b = 0] = ctx.getImageData(0, 0, 1, 1).data;
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

function Swatch({ token, label, className }: { token: string; label: string; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [hex, setHex] = useState('');
  useEffect(() => {
    const read = () => {
      if (ref.current) setHex(toHex(getComputedStyle(ref.current).backgroundColor));
    };
    read();
    const observer = new MutationObserver(read);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    });
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    media.addEventListener('change', read);
    return () => {
      observer.disconnect();
      media.removeEventListener('change', read);
    };
  }, []);
  return (
    <div className={className}>
      <div
        ref={ref}
        className="h-14 rounded-control border border-rule-strong"
        style={{ background: `var(${token})` }}
      />
      <p className="mt-1.5 text-ui font-medium">{label}</p>
      <p className="tabular text-meta text-text-2">
        {token} {hex}
      </p>
    </div>
  );
}

function Section({
  title,
  intro,
  children,
}: {
  title: string;
  intro: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h2 className="text-name font-semibold">{title}</h2>
      <p className="mt-1 max-w-[64ch] text-ui text-text-2">{intro}</p>
      <div className="mt-5">{children}</div>
    </section>
  );
}

const INK_NAMES = [
  'vermilion',
  'copper',
  'ochre',
  'moss',
  'viridian',
  'teal',
  'cerulean',
  'aubergine',
  'plum',
  'rose',
];

const GAP_SAMPLES = [
  MINUTE,
  10 * MINUTE,
  HOUR,
  6 * HOUR,
  DAY,
  3 * DAY,
  7 * DAY,
  21 * DAY,
  90 * DAY,
];

function FoldDemo() {
  const [level, setLevel] = useState(0);
  const pane = (name: string, n: number) => (
    <div className="grid h-full place-items-center text-center">
      <div>
        <p className="text-name font-semibold">{name}</p>
        <p className="mt-1 text-ui text-text-2">Level {n} of 3</p>
      </div>
    </div>
  );
  return (
    <div className="flex items-start gap-4">
      <div className="h-44 w-[320px] overflow-hidden rounded-control border border-rule-strong">
        <FoldStack
          level={level}
          panes={[pane('People', 1), pane('Conversations', 2), pane('Conversation', 3)]}
        />
      </div>
      <div className="flex flex-col gap-2">
        <Button
          variant="ink"
          disabled={level >= 2}
          onClick={() => setLevel((l) => Math.min(2, l + 1))}
        >
          Go deeper
        </Button>
        <Button disabled={level <= 0} onClick={() => setLevel((l) => Math.max(0, l - 1))}>
          Go back
        </Button>
      </div>
    </div>
  );
}

export function DesignSpecimen() {
  const { tier, spec } = useMotionGovernor();
  const [stamps, setStamps] = useState(0);
  const [moving, setMoving] = useState(false);
  const [segment, setSegment] = useState<'letter' | 'original'>('letter');
  const [chip, setChip] = useState(true);
  const [query, setQuery] = useState('');

  return (
    <div className="scroll-y h-full bg-pane px-10 py-8" data-testid="design-specimen">
      <div className="mx-auto max-w-[980px] space-y-14">
        <header>
          <h1 className="text-display font-semibold tracking-[-0.015em]">Design specimen</h1>
          <p className="mt-2 max-w-[62ch] text-body text-text-2">
            Blue-black ink on a cool desk. Messages are paper sheets. Colour belongs to people, not
            to the brand. The airmail stripe appears only where something needs you.
          </p>
        </header>

        <Section
          title="Surfaces and ink"
          intro="One declaration per token; light-dark() follows the system theme or the setting. Paper stays light in dark mode, because HTML mail is written for light backgrounds."
        >
          <div className="grid grid-cols-4 gap-4 sm:grid-cols-6">
            <Swatch token="--desk" label="Desk" />
            <Swatch token="--pane" label="Pane" />
            <Swatch token="--sheet" label="Sheet" />
            <Swatch token="--paper" label="Paper" />
            <Swatch token="--text" label="Ink" />
            <Swatch token="--text-2" label="Ink, secondary" />
            <Swatch token="--rule" label="Rule" />
            <Swatch token="--blue" label="Quink blue" />
            <Swatch token="--red" label="Postmark red" />
            <Swatch token="--airmail-blue" label="Airmail blue" />
            <Swatch token="--paper-text" label="Paper text" />
            <Swatch token="--text-3" label="Disabled only" />
          </div>
        </Section>

        <Section
          title="People are stamps"
          intro="Ten inks, one per person, stable by address. People are perforated stamps; services and lists are round postmarks; you are plain ink. Initials handle Latin, CJK, Arabic, Hebrew and emoji."
        >
          <div className="grid grid-cols-5 gap-5 sm:grid-cols-10">
            {INK_NAMES.map((name, ink) => (
              <div key={name} className="text-center">
                <Stamp
                  name={name.charAt(0).toUpperCase() + name.slice(1)}
                  ink={ink}
                  size={56}
                  className="mx-auto"
                />
                <p className="mt-1.5 text-meta text-text-2">{name}</p>
              </div>
            ))}
          </div>
          <div className="mt-8 flex flex-wrap items-end gap-6">
            {[24, 32, 40, 56, 72].map((size) => (
              <div key={size} className="text-center">
                <Stamp name="Maya Okafor" ink={6} size={size} className="mx-auto" />
                <p className="tabular mt-1.5 text-meta text-text-2">{size}px</p>
              </div>
            ))}
            <div className="mx-2 h-12 w-px bg-rule-strong" />
            {(
              [
                ['Person', 'person', 'Dad'],
                ['Service', 'service', 'DHL Parcel'],
                ['List', 'list', 'The Weekly Dispatch'],
                ['You', 'self', 'You'],
              ] as const
            ).map(([label, kind, name]) => (
              <div key={label} className="text-center">
                <Stamp name={name} ink={2} kind={kind} size={48} className="mx-auto" />
                <p className="mt-1.5 text-meta text-text-2">{label}</p>
              </div>
            ))}
            <div className="mx-2 h-12 w-px bg-rule-strong" />
            {['田中 弘', 'علي حسن', 'Noa Katz (נועה כץ)', 'Sofia 🎉 Marchetti'].map((name) => (
              <div key={name} className="text-center">
                <Stamp name={name} ink={name.length % 10} size={48} className="mx-auto" />
                <p className="mt-1.5 text-meta text-text-2">{initials(name)}</p>
              </div>
            ))}
          </div>
        </Section>

        <Section
          title="Type"
          intro="Bricolage Grotesque for the interface and names, with its width and optical-size axes. Literata for message text, because letters are read in a text face. Times and counts use tabular figures."
        >
          <div className="grid grid-cols-2 gap-8">
            <div className="space-y-3">
              <p className="text-display font-semibold tracking-[-0.015em]">Maya Okafor</p>
              <p className="text-title font-semibold tracking-[-0.01em]">Q3 launch checklist</p>
              <p className="text-name font-semibold">Design lead, Northwind</p>
              <p className="text-body">Needs a reply, 14 messages over 5 weeks.</p>
              <p className="text-ui text-text-2">
                Tooltip is on staging. Could you try it on your phone?
              </p>
              <p className="tabular font-stretch-semi-condensed text-meta text-text-2">
                09:12 &nbsp; Tue 10:42 &nbsp; 14 messages &nbsp; 1,284 people
              </p>
            </div>
            <div className="sheet p-5">
              <div className="letter">
                <p>
                  Hi, attaching the checklist for the Q3 launch. Two blockers so far: the pricing
                  page copy isn’t signed off, and Dan still needs to send the final screenshots.
                </p>
                <p>
                  Can you take a look before Thursday’s sync? If anything is missing, add it
                  straight into the doc.
                </p>
              </div>
            </div>
          </div>
        </Section>

        <Section
          title="Controls"
          intro="Controls are rounder (8 px) than content (4 px). Active filters are solid ink; the primary action is the only blue fill."
        >
          <div className="flex flex-wrap items-center gap-3">
            <Button variant="primary">Send</Button>
            <Button variant="ink">Allow</Button>
            <Button>Cancel</Button>
            <Button variant="primary" disabled>
              Disabled
            </Button>
            <Chip active={chip} onClick={() => setChip((c) => !c)}>
              Needs reply
            </Chip>
            <Chip count={3}>Waiting on them</Chip>
            <Segmented
              label="Message view"
              value={segment}
              onChange={setSegment}
              options={[
                { value: 'letter', label: 'Letter' },
                { value: 'original', label: 'Original' },
              ]}
            />
            <span className="inline-flex items-center gap-1">
              <Kbd>Ctrl</Kbd>
              <Kbd>K</Kbd>
            </span>
            <SearchField className="w-56" label="Search people" value={query} onChange={setQuery} />
          </div>
        </Section>

        <Section
          title="The strand"
          intro="The height between two messages is the time between them, on a log scale: a fast volley stays tight, a long silence stretches. Gaps of three days or more turn dashed and get a sentence."
        >
          <div className="space-y-1.5">
            {GAP_SAMPLES.map((gap) => (
              <div key={gap} className="flex items-center gap-4">
                <span className="tabular w-32 text-ui text-text-2">{gapLabel(gap)}</span>
                <span className="tabular w-14 text-ui">{Math.round(gapPx(gap))} px</span>
                <span
                  className="h-2 rounded-full bg-blue"
                  style={{ width: `${(gapPx(gap) / GAP_MAX_PX) * 320}px` }}
                />
              </div>
            ))}
          </div>
        </Section>

        <Section
          title="Motion"
          intro={`Playing at the ${tier} tier. Fold moves between levels, Stamp confirms a commit, Settle moves list items, Draw grows the strand. Lower tiers keep opacity and drop movement.`}
        >
          <div className="flex flex-wrap items-start gap-10">
            <FoldDemo />
            <div>
              <div className="relative flex h-[76px] items-center">
                <Button variant="ink" onClick={() => setStamps((n) => n + 1)}>
                  Play stamp
                </Button>
                {stamps > 0 && (
                  <div className="absolute left-28 -top-1">
                    <Postmark key={stamps} label="Sent" />
                  </div>
                )}
              </div>
              <p className="mt-1 text-meta text-text-2">
                Stamp: {spec.stamp}. Fold: {spec.fold ? 'hinge' : 'crossfade'}. Draw:{' '}
                {spec.draw ? 'on' : 'off'}.
              </p>
            </div>
            <div className="w-56">
              <Airmail moving={moving} />
              <Button className="mt-3" onClick={() => setMoving((m) => !m)}>
                {moving ? 'Stop the stripe' : 'Syncing'}
              </Button>
              <p className="mt-2 text-meta text-text-2">
                The stripe moves only while work is happening.
              </p>
            </div>
          </div>
        </Section>
      </div>
    </div>
  );
}
