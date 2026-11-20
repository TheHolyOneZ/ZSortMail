export interface SnoozeChoice {
  label: string;
  at: number;
}

function atHour(base: Date, hour: number): Date {
  const d = new Date(base);
  d.setHours(hour, 0, 0, 0);
  return d;
}

export function snoozeChoices(nowMs: number): SnoozeChoice[] {
  const now = new Date(nowMs);
  const out: SnoozeChoice[] = [];

  const later = new Date(nowMs + 3 * 3_600_000);
  later.setMinutes(Math.ceil(later.getMinutes() / 15) * 15, 0, 0);
  if (later.getDate() === now.getDate()) out.push({ label: 'Later today', at: later.getTime() });

  const tomorrow = atHour(new Date(nowMs + 86_400_000), 8);
  out.push({ label: 'Tomorrow morning', at: tomorrow.getTime() });

  if (now.getDay() >= 1 && now.getDay() <= 4) {
    const saturday = atHour(new Date(nowMs + (6 - now.getDay()) * 86_400_000), 9);
    out.push({ label: 'This weekend', at: saturday.getTime() });
  }

  const toMonday = (1 - now.getDay() + 7) % 7 || 7;
  const monday = atHour(new Date(nowMs + toMonday * 86_400_000), 8);
  if (monday.getTime() - tomorrow.getTime() > 3_600_000)
    out.push({ label: 'Next week', at: monday.getTime() });

  return out.filter((c) => c.at > nowMs);
}
