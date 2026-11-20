import { DotsThree, EyeSlash, Prohibit, Tray, UserCheck } from '@phosphor-icons/react';
import { AnimatePresence, motion, useMotionValue, useTransform } from 'motion/react';
import { useEffect, useState } from 'react';
import { act, undo } from '@/lib/actions';
import { api } from '@/lib/api';
import type { NewSender, SenderDecision } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { formatMessageTime, plural } from '@/lib/format';
import { stampColor } from '@/lib/ink';
import { useNewSenders } from '@/lib/queries';
import { useShortcuts } from '@/lib/shortcuts';
import { useMotionGovernor } from '@/motion/MotionProvider';
import { ease } from '@/motion/tokens';
import { Button, Kbd } from '@/ui/controls';
import { ZMark } from '@/ui/icons';
import { Menu } from '@/ui/Menu';
import { Stamp } from '@/ui/Stamp';

interface Exit {
  x: number;
  y: number;
}

const DIRECTION: Record<SenderDecision, Exit> = {
  allow: { x: 1, y: 0 },
  hide: { x: -1, y: 0 },
  block: { x: 0, y: -1 },
};

const SWIPE_PX = 110;

export function NewSendersScreen() {
  const { data: senders = [], isSuccess } = useNewSenders();
  const { spec } = useMotionGovernor();
  const [gone, setGone] = useState<ReadonlySet<number>>(() => new Set());
  const [exit, setExit] = useState<Exit>({ x: 1, y: 0 });

  useEffect(() => {
    setGone((prev) => {
      const present = new Set(senders.map((s) => s.person.id));
      const next = new Set([...prev].filter((id) => present.has(id)));
      return next.size === prev.size ? prev : next;
    });
  }, [senders]);

  const waiting = senders.filter((s) => !gone.has(s.person.id));
  const top = waiting[0];

  async function decide(sender: NewSender, decision: SenderDecision) {
    setExit(DIRECTION[decision]);
    setGone((prev) => new Set(prev).add(sender.person.id));
    const ok = await act(() => api.decideSender(sender.person.id, decision));
    if (!ok) {
      setGone((prev) => {
        const next = new Set(prev);
        next.delete(sender.person.id);
        return next;
      });
    }
  }

  async function decideDomain(sender: NewSender, decision: SenderDecision) {
    setExit(DIRECTION[decision]);
    const sameDomain = waiting.filter((s) => s.domain === sender.domain).map((s) => s.person.id);
    setGone((prev) => new Set([...prev, ...sameDomain]));
    const ok = await act(() => api.decideDomain(sender.domain, decision));
    if (!ok) setGone(new Set());
  }

  useShortcuts([
    { key: 'a', run: () => top && void decide(top, 'allow') },
    { key: 'ArrowRight', run: () => top && void decide(top, 'allow') },
    { key: 'h', run: () => top && void decide(top, 'hide') },
    { key: 'ArrowLeft', run: () => top && void decide(top, 'hide') },
    { key: 'b', run: () => top && void decide(top, 'block') },
    { key: 'ArrowUp', run: () => top && void decide(top, 'block') },
    { key: 'z', run: () => void undo() },
  ]);

  return (
    <section className="flex h-full min-h-0 flex-col bg-pane" aria-label="New senders">
      <header className="shrink-0 px-8 pt-7">
        <h1 className="text-title font-semibold tracking-[-0.01em]">New senders</h1>
        <p className="mt-1 max-w-[56ch] text-ui text-text-2">
          {waiting.length > 0
            ? `${plural(waiting.length, 'person', 'people')} you have not heard from before. Choose what each one is.`
            : 'Anyone who writes to you for the first time waits here until you decide.'}
        </p>
      </header>

      <div className="relative mx-auto flex min-h-0 w-full max-w-[540px] flex-1 flex-col items-center justify-center px-6 py-6">
        {isSuccess && !top && <Empty />}
        {top && (
          <div className="relative h-[330px] w-full" aria-live="polite">
            <AnimatePresence custom={exit} initial={false}>
              {waiting
                .slice(0, 3)
                .reverse()
                .map((sender) => {
                  const depth = waiting.indexOf(sender);
                  return (
                    <Card
                      key={sender.person.id}
                      sender={sender}
                      depth={depth}
                      animateExit={spec.flight}
                      onDecide={(d) => void decide(sender, d)}
                    />
                  );
                })}
            </AnimatePresence>
          </div>
        )}

        {top && (
          <div className="mt-6 flex w-full flex-col items-center gap-3">
            <div className="flex flex-wrap items-center justify-center gap-3">
              <Button
                className="h-10 gap-2 px-4 text-body"
                onClick={() => void decide(top, 'hide')}
              >
                <EyeSlash size={18} />
                Hide <Kbd>H</Kbd>
              </Button>
              <Button
                className="h-10 gap-2 px-4 text-body"
                onClick={() => void decide(top, 'block')}
              >
                <Prohibit size={18} />
                Block <Kbd>B</Kbd>
              </Button>
              <Button
                variant="primary"
                className="h-10 gap-2 px-5 text-body"
                onClick={() => void decide(top, 'allow')}
              >
                <UserCheck size={18} weight="fill" />
                Allow <Kbd>A</Kbd>
              </Button>
            </div>
            <div className="flex items-center gap-2 text-meta text-text-2">
              <span>Drag the card, or press the arrow keys. </span>
              <Kbd>Z</Kbd>
              <span>undoes.</span>
              <Menu
                label={`More choices for ${top.domain}`}
                trigger={<DotsThree size={18} weight="bold" />}
                align="start"
                items={[
                  {
                    label: `Allow everyone at ${top.domain}`,
                    onSelect: () => void decideDomain(top, 'allow'),
                  },
                  {
                    label: `Hide everyone at ${top.domain}`,
                    onSelect: () => void decideDomain(top, 'hide'),
                  },
                  {
                    label: `Block everyone at ${top.domain}`,
                    danger: true,
                    onSelect: () => void decideDomain(top, 'block'),
                  },
                ]}
              />
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

function Empty() {
  return (
    <div className="max-w-[40ch] text-center">
      <ZMark size={44} className="mx-auto text-text-3" />
      <h2 className="mt-4 text-name font-semibold">No one is waiting</h2>
      <p className="mt-1 text-ui text-text-2">
        The first time someone new writes to you, they appear here. You decide whether they join
        People, stay out of sight, or are blocked.
      </p>
      <p className="mt-3 inline-flex items-center gap-2 text-meta text-text-2">
        <Tray size={14} /> Nothing is lost while a sender waits. Their mail is kept.
      </p>
    </div>
  );
}

const cardVariants = {
  enter: (c: { depth: number }) => ({ opacity: 0, scale: 0.94, y: c.depth * 14 + 24 }),
  center: (c: { depth: number }) => ({
    opacity: 1 - c.depth * 0.12,
    scale: 1 - c.depth * 0.045,
    y: c.depth * 16,
    x: 0,
    rotate: 0,
  }),
  exit: (c: Exit) => ({
    x: c.x * 720,
    y: c.y * 560,
    rotate: c.x * 9,
    opacity: 0,
    transition: { duration: 0.3, ease: ease.fold },
  }),
};

function Card({
  sender,
  depth,
  animateExit,
  onDecide,
}: {
  sender: NewSender;
  depth: number;
  animateExit: boolean;
  onDecide: (decision: SenderDecision) => void;
}) {
  const top = depth === 0;
  const x = useMotionValue(0);
  const y = useMotionValue(0);
  const rotate = useTransform(x, [-320, 320], [-7, 7]);
  const allowOpacity = useTransform(x, [30, SWIPE_PX + 30], [0, 1]);
  const hideOpacity = useTransform(x, [-SWIPE_PX - 30, -30], [1, 0]);
  const blockOpacity = useTransform(y, [-SWIPE_PX - 30, -30], [1, 0]);
  const ink = stampColor(sender.person.ink);
  const since = formatMessageTime(sender.first_at, now());

  return (
    <motion.article
      aria-hidden={!top}
      aria-label={top ? `${sender.person.name}, new sender` : undefined}
      className={`sheet absolute inset-x-0 top-0 h-[300px] origin-bottom overflow-hidden px-6 pt-5 pb-4 ${top ? '' : '[&>*]:invisible'}`}
      style={{ x, y, rotate, zIndex: 10 - depth, '--stamp': ink } as never}
      custom={animateExit ? { depth } : { x: 0, y: 0, depth }}
      variants={cardVariants}
      initial="enter"
      animate="center"
      exit={animateExit ? 'exit' : { opacity: 0, transition: { duration: 0 } }}
      drag={top}
      dragSnapToOrigin
      dragElastic={0.55}
      dragMomentum={false}
      whileDrag={{ cursor: 'grabbing' }}
      onDragEnd={(_, info) => {
        if (info.offset.x > SWIPE_PX || info.velocity.x > 700) onDecide('allow');
        else if (info.offset.x < -SWIPE_PX || info.velocity.x < -700) onDecide('hide');
        else if (info.offset.y < -SWIPE_PX || info.velocity.y < -700) onDecide('block');
      }}
    >
      <div className="flex items-start gap-4">
        <Stamp
          name={sender.person.name}
          ink={sender.person.ink}
          kind={sender.person.kind}
          size={56}
        />
        <div className="min-w-0 flex-1">
          <h2 dir="auto" className="truncate text-title font-semibold tracking-[-0.01em]">
            {sender.person.name}
          </h2>
          <p dir="ltr" className="mt-0.5 truncate text-ui text-paper-text-2 select-text">
            {sender.address}
          </p>
        </div>
      </div>

      <h3 dir="auto" className="mt-5 line-clamp-2 text-name font-semibold">
        {sender.subject || 'No subject'}
      </h3>
      <p
        dir="auto"
        className="letter mt-1 line-clamp-3 !text-[15px] !leading-[1.55] text-paper-text-2"
      >
        {sender.snippet}
      </p>

      <p className="absolute right-6 bottom-4 left-6 text-meta text-paper-text-2">
        {plural(sender.message_count, 'message')}
        {sender.file_count > 0 ? `, ${plural(sender.file_count, 'file')}` : ''}. First wrote {since}
        .{sender.bulk ? ' Looks like a newsletter or a shop.' : ''}
      </p>

      {top && (
        <>
          <Verdict
            style={{ opacity: allowOpacity, right: 18, top: 62 }}
            label="Allow"
            tone="var(--stamp-4)"
            tilt={8}
          />
          <Verdict
            style={{ opacity: hideOpacity, left: 18, top: 62 }}
            label="Hide"
            tone="var(--stamp-2)"
            tilt={-8}
          />
          <Verdict
            style={{ opacity: blockOpacity, left: '50%', top: 18, translateX: '-50%' }}
            label="Block"
            tone="var(--red)"
            tilt={-4}
          />
        </>
      )}
    </motion.article>
  );
}

function Verdict({
  label,
  tone,
  tilt,
  style,
}: {
  label: string;
  tone: string;
  tilt: number;
  style: Record<string, unknown>;
}) {
  return (
    <motion.span
      aria-hidden="true"
      className="pointer-events-none absolute rounded-[6px] border-[3px] px-3 py-0.5 text-name font-bold tracking-wide"
      style={{ ...style, color: tone, borderColor: tone, rotate: tilt }}
    >
      {label}
    </motion.span>
  );
}
