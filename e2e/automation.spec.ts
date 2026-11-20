import { expect, type Page, test } from '@playwright/test';

test.describe.configure({ mode: 'serial' });

const NOW = '2026-11-18T10:00:00+01:00';
const url = (hash: string) => `/?now=${encodeURIComponent(NOW)}#${hash}`;

test.use({ viewport: { width: 1360, height: 860 } });

async function ipc<T>(page: Page, cmd: string, data: object = {}): Promise<T> {
  const response = await page.request.post(`/ipc/${cmd}`, { data });
  return (await response.json()) as T;
}

function watchForErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('Failed to load resource'))
      errors.push(`console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  return errors;
}

test('a watch made from the palette waits, and a watch can count mail that is already here', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  await page.keyboard.press('Control+k');
  const palette = page.getByRole('dialog', { name: 'Search and commands' });
  await palette.getByRole('combobox').fill('watch passport appointment');
  await expect(
    palette.getByRole('option', { name: /Wait for “passport appointment”/ }),
  ).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/waiting/);
  await expect(page.getByText('Waiting for “passport appointment”.')).toBeVisible();
  const waiting = page.getByRole('region', { name: /^Waiting \(1\)/ });
  await expect(
    waiting.getByRole('heading', { level: 3, name: 'passport appointment' }),
  ).toBeVisible();
  await expect(waiting.getByText(/waiting (moments|\d+ min)/)).toBeVisible();

  await page.getByRole('button', { name: 'Watch for an email' }).click();
  const dialog = page.getByRole('dialog', { name: 'Wait for an email' });
  await dialog.getByLabel('What are you waiting for?').fill('xbox 394');
  await expect(dialog.getByText(/1 message here already match/)).toBeVisible();
  await dialog.getByLabel('Also count mail that is already here').check();
  await page.screenshot({ path: 'test-results/screens/watch-dialog.png' });
  await dialog.getByRole('button', { name: 'Start watching' }).click();
  await expect(page.getByText(/already here counted/)).toBeVisible();

  const arrived = page.getByRole('region', { name: 'Arrived' });
  await expect(arrived.getByRole('heading', { level: 3, name: 'xbox 394' })).toBeVisible();
  await expect(arrived.getByRole('button', { name: /Tracking JD014600003394512/ })).toBeVisible();
  await expect(arrived.getByRole('button', { name: /Order 394-2211/ })).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/waiting.png' });
  await arrived.getByRole('button', { name: 'Open' }).click();
  await expect(
    page.getByRole('heading', { level: 2, name: 'Your parcel is on its way' }),
  ).toBeVisible();

  const watches = await ipc<{ id: number }[]>(page, 'list_watches');
  for (const w of watches) await ipc(page, 'delete_watch', { id: w.id });
  expect(errors).toEqual([]);
});

test('an urgent watch is marked with the airmail edge, and can be paused and resumed', async ({
  page,
}) => {
  await page.goto(url('/waiting'));
  await page.getByRole('button', { name: 'Watch for an email' }).click();
  const dialog = page.getByRole('dialog', { name: 'Wait for an email' });
  await dialog.getByLabel('What are you waiting for?').fill('visa appointment');
  await dialog.getByLabel(/^Urgent/).check();
  await dialog.getByLabel(/Keep watching/).check();
  await dialog.getByRole('button', { name: 'Start watching' }).click();
  const row = page
    .getByRole('region', { name: /^Waiting/ })
    .getByRole('listitem')
    .first();
  await expect(row.getByText('keeps watching, urgent')).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/waiting-urgent.png' });
  await row.getByRole('button', { name: 'Pause' }).click();
  const paused = page.getByRole('region', { name: 'Paused and ended' });
  await expect(paused.getByText('paused', { exact: true })).toBeVisible();
  await paused.getByRole('button', { name: 'Watch again' }).click();
  await expect(
    page
      .getByRole('region', { name: /^Waiting/ })
      .getByRole('heading', { level: 3, name: 'visa appointment' }),
  ).toBeVisible();
  for (const w of await ipc<{ id: number }[]>(page, 'list_watches'))
    await ipc(page, 'delete_watch', { id: w.id });
});

test('a rule shows what it would catch, applies to existing mail in one step, and undoes in one step', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/rules'));
  await expect(page.getByRole('heading', { level: 2, name: 'No rules yet' })).toBeVisible();
  await page.getByRole('button', { name: 'New rule' }).click();
  const dialog = page.getByRole('dialog', { name: 'New rule' });
  await dialog.getByLabel('When mail matches').fill('category:newsletter');
  await expect(dialog.getByText(/This would catch 1 message here/)).toBeVisible();
  await expect(dialog.getByText(/Issue 212/)).toBeVisible();
  await dialog.getByLabel(/^Archive/).check();
  await dialog.getByLabel('Mark as read').check();
  await page.screenshot({ path: 'test-results/screens/rule-dialog.png' });
  await dialog.getByRole('button', { name: 'Save rule' }).click();

  const list = page.getByRole('list', { name: 'Rules in order' });
  await expect(list.getByText('Has not matched anything yet')).toBeVisible();
  const dispatch = (
    await ipc<{ id: number; unread: number }[]>(page, 'list_people', {
      filter: 'all',
      query: 'Dispatch',
    })
  )[0];
  expect(dispatch?.unread).toBe(1);

  await list.getByRole('button', { name: 'Apply to existing' }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Rule applied to 1 conversation.' }),
  ).toBeVisible();
  await expect
    .poll(
      async () =>
        (
          await ipc<{ id: number; unread: number }[]>(page, 'list_people', {
            filter: 'all',
            query: 'Dispatch',
          })
        )[0]?.unread,
    )
    .toBe(0);
  await expect(list.getByText(/Caught 1 message/)).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect
    .poll(
      async () =>
        (
          await ipc<{ id: number; unread: number }[]>(page, 'list_people', {
            filter: 'all',
            query: 'Dispatch',
          })
        )[0]?.unread,
    )
    .toBe(1);

  await page.getByRole('button', { name: 'Edit rule' }).click();
  const edit = page.getByRole('dialog', { name: 'Edit rule' });
  await edit.getByLabel('When mail matches').fill('   ');
  await expect(edit.getByRole('button', { name: 'Save rule' })).toBeDisabled();
  await edit.getByRole('button', { name: 'Cancel' }).click();

  const enabled = page.getByRole('switch', { name: /^Run / });
  await enabled.click();
  await expect(enabled).not.toBeChecked();
  await expect
    .poll(async () => (await ipc<{ enabled: boolean }[]>(page, 'list_rules'))[0]?.enabled)
    .toBe(false);
  await page.getByRole('button', { name: 'Delete rule' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'No rules yet' })).toBeVisible();
  expect(errors).toEqual([]);
});

test('Today lists what needs you, and a snoozed conversation leaves and can be brought back', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/today'));
  await expect(page.getByRole('heading', { level: 1, name: 'Today' })).toBeVisible();
  const needs = page.getByRole('region', { name: 'Needs you' });
  const before = (await ipc<{ needs_reply: unknown[] }>(page, 'get_today')).needs_reply.length;
  await expect(needs.getByRole('listitem')).toHaveCount(before);
  await page.screenshot({ path: 'test-results/screens/today.png' });
  const first = needs.getByRole('listitem').first();
  const name = (await first.locator('span[dir="auto"]').first().textContent()) ?? '';

  await first.hover();
  await first.getByRole('button', { name: 'Snooze' }).click();
  await page.getByRole('menuitem', { name: /^Tomorrow morning/ }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Snoozed until/ })).toBeVisible();
  await expect(needs.getByRole('listitem')).toHaveCount(before - 1);
  const snoozed = page.getByRole('region', { name: 'Snoozed' });
  await expect(snoozed.getByText(name).first()).toBeVisible();
  await expect(snoozed.getByText(/^back /)).toBeVisible();

  await snoozed.getByRole('listitem').first().hover();
  await snoozed.getByRole('button', { name: 'Bring back' }).click();
  await expect(needs.getByRole('listitem')).toHaveCount(before);
  expect(errors).toEqual([]);
});

test('Mail is one flat inbox with categories kept apart', async ({ page }) => {
  await page.goto(url('/river'));
  await expect(page.getByRole('heading', { level: 1, name: 'Mail' })).toBeVisible();
  const lines = page.getByRole('list', { name: 'Conversations' }).getByRole('listitem');
  await expect(lines.first()).toBeVisible();
  const all = await lines.count();
  expect(all).toBeGreaterThan(15);
  await page.getByRole('button', { name: /^Newsletters/ }).click();
  await expect(lines).toHaveCount(1);
  await expect(lines.first()).toContainText('The Weekly Dispatch');
  await page.getByRole('button', { name: /^All \d/ }).click();
  await expect(lines).toHaveCount(all);
  await page.screenshot({ path: 'test-results/screens/mail.png' });
  await expect(page.getByText('Priya Raman')).toHaveCount(0);
});

test('notification settings are remembered and the log starts empty and explains itself', async ({
  page,
}) => {
  await page.goto(url('/settings'));
  const quiet = page.getByRole('switch', { name: 'Quiet hours' });
  await quiet.click();
  await expect(quiet).toBeChecked();
  await expect
    .poll(
      async () =>
        (
          await ipc<{ quiet: { from: number; to: number } | null }>(
            page,
            'get_notification_settings',
          )
        ).quiet,
    )
    .toEqual({ from: 1320, to: 420 });
  const focus = page.getByRole('switch', { name: /Focus mode/ });
  await focus.click();
  await expect(focus).toBeChecked();
  await expect
    .poll(async () => (await ipc<{ focus: boolean }>(page, 'get_notification_settings')).focus)
    .toBe(true);
  await page.screenshot({
    path: 'test-results/screens/settings-notifications.png',
    fullPage: true,
  });
  await ipc(page, 'save_notification_settings', {
    settings: { enabled: true, quiet: null, focus: false, important: false },
  });

  await page.getByRole('link', { name: /See what was notified/ }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Notification log' })).toBeVisible();
  await expect(page.getByText('Nothing has been notified yet.')).toBeVisible();
});
