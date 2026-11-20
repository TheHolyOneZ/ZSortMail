import { Paperclip } from '@phosphor-icons/react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { api } from '@/lib/api';
import type { TimelineEntry } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { Button } from '@/ui/controls';

function dayLabel(ms: number, nowTs: number): string {
  const d = new Date(ms);
  const t = new Date(nowTs);
  const days = Math.round(
    (new Date(t.getFullYear(), t.getMonth(), t.getDate()).getTime() -
      new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()) /
      86_400_000,
  );
  if (days === 0) return 'Today';
  if (days === 1) return 'Yesterday';
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: d.getFullYear() === t.getFullYear() ? undefined : 'numeric',
  }).format(d);
}

const clock = (ms: number) =>
  new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(ms);

export function TimelineTab({ personId }: { personId: number }) {
  const navigate = useNavigate();
  const { data, fetchNextPage, hasNextPage, isFetchingNextPage, isSuccess } = useInfiniteQuery({
    queryKey: ['timeline', personId],
    queryFn: ({ pageParam }) => api.getTimeline(personId, pageParam),
    initialPageParam: undefined as number | undefined,
    getNextPageParam: (last) => (last.length < 60 ? undefined : last[last.length - 1]?.sent_at),
  });
  const entries: TimelineEntry[] = data?.pages.flat() ?? [];
  const nowTs = now();

  if (isSuccess && entries.length === 0) {
    return <p className="px-4 py-6 text-ui text-text-2">No messages yet.</p>;
  }

  const groups: { label: string; items: TimelineEntry[] }[] = [];
  for (const e of entries) {
    const label = dayLabel(e.sent_at, nowTs);
    const last = groups[groups.length - 1];
    if (last?.label === label) last.items.push(e);
    else groups.push({ label, items: [e] });
  }

  return (
    <div>
      {groups.map((g) => (
        <section key={g.label} aria-label={g.label}>
          <h3 className="sticky top-0 z-10 bg-pane px-4 py-1.5 text-meta font-medium text-text-2">
            {g.label}
          </h3>
          <ul>
            {g.items.map((e) => (
              <li key={e.message_id}>
                <button
                  type="button"
                  onClick={() =>
                    void navigate({
                      to: '/people/$personId/c/$conversationId',
                      params: {
                        personId: String(personId),
                        conversationId: String(e.conversation_id),
                      },
                    })
                  }
                  className="flex w-full items-start gap-3 px-4 py-2.5 text-left transition-colors hover:bg-hover"
                >
                  <time className="tabular w-11 shrink-0 pt-0.5 text-meta text-text-2">
                    {clock(e.sent_at)}
                  </time>
                  <span className="min-w-0 flex-1">
                    <span className="flex items-baseline gap-2">
                      <span
                        className={cn(
                          'shrink-0 text-ui',
                          e.unread ? 'font-semibold' : 'font-medium',
                          e.from_me ? 'text-blue' : 'text-text',
                        )}
                      >
                        {e.from_me ? 'You' : e.from_name.split(' ')[0]}
                      </span>
                      <span dir="auto" className="truncate text-ui text-text-2">
                        {e.subject}
                      </span>
                      {e.files > 0 && (
                        <Paperclip
                          size={12}
                          className="shrink-0 text-text-2"
                          aria-label="Has files"
                        />
                      )}
                    </span>
                    <span dir="auto" className="line-clamp-2 text-ui">
                      {e.snippet}
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
      {hasNextPage && (
        <div className="px-4 py-3">
          <Button disabled={isFetchingNextPage} onClick={() => void fetchNextPage()}>
            {isFetchingNextPage ? 'Loading' : 'Show older'}
          </Button>
        </div>
      )}
    </div>
  );
}
