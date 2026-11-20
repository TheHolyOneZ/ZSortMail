import AxeBuilder from '@axe-core/playwright';
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

async function personId(page: Page, name: string): Promise<number> {
  const people = await ipc<{ id: number }[]>(page, 'list_people', { filter: 'all', query: name });
  if (!people[0]) throw new Error(`no demo person matches ${name}`);
  return people[0].id;
}

async function clearDrafts(page: Page) {
  for (const d of await ipc<{ id: number }[]>(page, 'list_drafts')) {
    await ipc(page, 'delete_draft', { id: d.id });
  }
}

test('C opens a new message, recipients become chips, Send puts it in the conversation, Undo takes it back', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  await page.keyboard.press('c');
  const composer = page.getByRole('region', { name: 'New message' });
  await expect(composer).toBeVisible();

  await page.keyboard.type('okafor');
  await expect(composer.getByRole('option', { name: /Maya Okafor/ })).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(composer.getByTitle('maya.okafor@northwind.example')).toBeVisible();
  await composer.getByLabel('Subject').fill('Lunch on Thursday?');
  await composer.getByRole('textbox', { name: 'Message' }).click();
  await page.keyboard.type('Are you free for lunch on Thursday?');
  await composer.getByRole('button', { name: 'Bold' }).click();
  await composer.getByRole('button', { name: 'Remind me if no reply' }).click();
  await page.getByRole('menuitem', { name: 'If no reply in 3 days' }).click();
  await expect(composer.getByText('Reminder in 3 days')).toBeVisible();
  await expect(composer.getByRole('status')).toHaveText('Draft saved');
  await page.screenshot({ path: 'test-results/screens/composer.png' });

  await composer.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(composer).toBeHidden();
  const toast = page.getByRole('status').filter({ hasText: /^Sent\./ });
  await expect(toast).toBeVisible();
  await expect(page.getByRole('heading', { level: 2, name: 'Lunch on Thursday?' })).toBeVisible();
  await expect(
    page
      .getByRole('region', { name: 'Conversation', exact: true })
      .getByText('Are you free for lunch on Thursday?'),
  ).toBeVisible();
  expect(await ipc<unknown[]>(page, 'list_nudges')).toHaveLength(1);

  await toast.getByRole('button', { name: 'Undo' }).click();
  const back = page.getByRole('region', { name: 'New message' });
  await expect(back.getByLabel('Subject')).toHaveValue('Lunch on Thursday?');
  await expect(back.getByText('Reminder in 3 days')).toBeVisible();
  expect(await ipc<unknown[]>(page, 'list_nudges')).toHaveLength(0);
  const maya = await personId(page, 'Okafor');
  const subjects = await ipc<{ subject: string }[]>(page, 'list_conversations', { personId: maya });
  expect(subjects.map((c) => c.subject)).not.toContain('Lunch on Thursday?');

  await back.getByRole('button', { name: 'Discard draft' }).click();
  await page
    .getByRole('dialog', { name: 'Discard this draft?' })
    .getByRole('button', { name: 'Discard' })
    .click();
  await expect(back).toBeHidden();
  expect(await ipc<unknown[]>(page, 'list_drafts')).toHaveLength(0);
  expect(errors).toEqual([]);
});

test('a reply opens with the subject and the message it answers; a draft nobody wrote in is not kept', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  const maya = await personId(page, 'Okafor');
  const convo = (
    await ipc<{ id: number; subject: string }[]>(page, 'list_conversations', { personId: maya })
  ).find((c) => c.subject === 'Coffee Thursday?');
  if (!convo) throw new Error('the demo has no Coffee Thursday? conversation');
  await page.goto(url(`/people/${maya}/c/${convo.id}`));
  await expect(page.getByRole('heading', { level: 2, name: 'Coffee Thursday?' })).toBeVisible();
  await page
    .getByRole('region', { name: 'Conversation', exact: true })
    .click({ position: { x: 5, y: 5 } });

  await page.keyboard.press('r');
  const reply = page.getByRole('region', { name: /^Reply to Maya/ });
  await expect(reply).toBeVisible();
  await expect(reply.getByLabel('Subject')).toHaveValue('Re: Coffee Thursday?');
  await expect(reply.getByText(/wrote:/)).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/composer-reply.png' });
  await page.keyboard.press('Escape');
  await expect(reply).toBeHidden();
  await expect(page.getByText('Draft saved.')).toHaveCount(0);
  expect(await ipc<unknown[]>(page, 'list_drafts')).toHaveLength(0);

  await page.keyboard.press('f');
  const forward = page.getByRole('region', { name: 'Forward' });
  await expect(forward.getByLabel('Subject')).toHaveValue('Fwd: Coffee Thursday?');
  await expect(forward.getByText(/Forwarded message/)).toBeVisible();
  await page.keyboard.type('someone@elsewhere.example');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Escape');
  await expect(forward).toBeHidden();
  await expect(page.getByRole('status').filter({ hasText: 'Draft saved.' })).toBeVisible();

  await page.goto(url('/drafts'));
  const list = page.getByRole('region', { name: 'Drafts', exact: true }).last();
  await expect(list.getByText('someone@elsewhere.example')).toBeVisible();
  await expect(list.getByText('Fwd: Coffee Thursday?')).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/drafts.png' });
  await list.getByRole('button', { name: /^Discard the draft to/ }).click({ force: true });
  await expect(page.getByText('Draft discarded.')).toBeVisible();
  await expect(page.getByRole('heading', { level: 2, name: 'No drafts' })).toBeVisible();
  expect(errors).toEqual([]);
});

test('files are attached by the button, listed with their size, and can be taken off again', async ({
  page,
}) => {
  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  await page.keyboard.press('c');
  const composer = page.getByRole('region', { name: 'New message' });
  await composer.getByLabel('Choose files').setInputFiles({
    name: 'plan.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('the plan, in words'),
  });
  const files = composer.getByRole('list', { name: 'Files to send' });
  await expect(files).toContainText('plan.txt');
  await expect(files).toContainText('18 B');
  expect(await ipc<{ attachments: unknown[] }[]>(page, 'list_drafts')).toHaveLength(1);
  await composer.getByRole('button', { name: 'Remove plan.txt' }).click();
  await expect(files).toHaveCount(0);

  await composer.getByLabel('Choose files').setInputFiles({
    name: '../../escape.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('x'),
  });
  await expect(composer.getByRole('list', { name: 'Files to send' })).toContainText('escape.txt');
  await composer.getByRole('button', { name: 'Discard draft' }).click();
  await page
    .getByRole('dialog', { name: 'Discard this draft?' })
    .getByRole('button', { name: 'Discard' })
    .click();
  await expect(composer).toBeHidden();
  expect(await ipc<unknown[]>(page, 'list_drafts')).toHaveLength(0);
});

test('an email can be sent later, and shows under Scheduled until its time', async ({ page }) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  await page.keyboard.press('c');
  const composer = page.getByRole('region', { name: 'New message' });
  await expect(composer).toBeVisible();
  await page.keyboard.type('later@elsewhere.example');
  await page.keyboard.press('Enter');
  await composer.getByLabel('Subject').fill('See you then');
  await composer.getByRole('textbox', { name: 'Message' }).click();
  await page.keyboard.type('Looking forward to it.');

  await composer.getByRole('button', { name: 'Send later' }).click();
  await page.getByRole('menuitem', { name: /^Tomorrow morning/ }).click();
  await expect(composer).toBeHidden();
  await expect(page.getByRole('status').filter({ hasText: /^Scheduled for/ })).toBeVisible();

  await page.goto(url('/drafts'));
  const scheduled = page.getByRole('region', { name: 'Scheduled' });
  await expect(scheduled.getByText('later@elsewhere.example')).toBeVisible();
  await expect(scheduled.getByText('See you then')).toBeVisible();

  await scheduled.getByRole('button', { name: /^later@elsewhere.example/ }).click();
  const open = page.getByRole('region', { name: 'New message' });
  await expect(open.getByText(/^Scheduled for/)).toBeVisible();
  await open.getByRole('button', { name: 'Cancel schedule' }).click();
  await expect(open.getByText(/^Scheduled for/)).toHaveCount(0);
  expect((await ipc<{ send_at: number | null }[]>(page, 'list_drafts'))[0]?.send_at).toBeNull();
  await open.getByRole('button', { name: 'Discard draft' }).click();
  await page
    .getByRole('dialog', { name: 'Discard this draft?' })
    .getByRole('button', { name: 'Discard' })
    .click();
  expect(await ipc<unknown[]>(page, 'list_drafts')).toHaveLength(0);
  expect(errors).toEqual([]);
});

test('the signature from Settings is added to new messages', async ({ page }) => {
  await page.goto(url('/settings'));
  await page.getByLabel('Signature').fill('Zo\nZSortMail, written on paper');
  await page.getByRole('combobox', { name: 'Undo send' }).focus();
  await expect
    .poll(async () => (await ipc<{ signature: string }>(page, 'get_compose_settings')).signature)
    .toBe('Zo\nZSortMail, written on paper');
  await page.getByRole('combobox', { name: 'Undo send' }).click();
  await page.getByRole('option', { name: '20 seconds' }).click();
  await expect
    .poll(
      async () => (await ipc<{ undo_seconds: number }>(page, 'get_compose_settings')).undo_seconds,
    )
    .toBe(20);

  await page.keyboard.press('Escape');
  await page.getByRole('link', { name: 'People' }).click();
  await page.keyboard.press('c');
  const composer = page.getByRole('region', { name: 'New message' });
  await expect(composer.getByRole('textbox', { name: 'Message' })).toContainText(
    'ZSortMail, written on paper',
  );
  await page.keyboard.press('Escape');
  await expect(composer).toBeHidden();
  await ipc(page, 'save_compose_settings', { settings: { signature: '', undo_seconds: 10 } });
  expect(await ipc<unknown[]>(page, 'list_drafts')).toHaveLength(0);
});

test('a routine is written once, previewed with today filled in, run on demand, skipped and removed', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/routines'));
  await expect(page.getByRole('heading', { level: 2, name: 'No routines yet' })).toBeVisible();
  await page.getByRole('button', { name: 'New routine' }).click();
  const dialog = page.getByRole('dialog', { name: 'New routine' });
  await dialog.getByLabel('Name').fill('Monday report');
  await dialog.getByLabel('To', { exact: true }).fill('boss@elsewhere.example');
  await page.keyboard.press('Enter');
  await dialog.getByLabel('Subject').fill('Report for {{weekday}}');
  await dialog.getByLabel('Text').fill('Numbers for {{date}}.');
  await expect(dialog.getByText(/Written today, it says/)).toContainText('Report for Wednesday');
  await expect(dialog.getByText(/Written today, it says/)).toContainText(
    'Numbers for Wed, Nov 18.',
  );
  await page.screenshot({ path: 'test-results/screens/routine-dialog.png' });

  await dialog.getByLabel('Text').fill('Numbers for {{nope}}.');
  await dialog.getByRole('button', { name: 'Save routine' }).click();
  await expect(dialog.getByRole('alert')).toContainText('{{nope}} is not a variable. Use date,');
  await dialog.getByLabel('Text').fill('Numbers for {{date}}.');
  await dialog.getByRole('button', { name: 'Save routine' }).click();
  await expect(dialog).toBeHidden();

  const row = page
    .getByRole('list', { name: /^Routines in order/ })
    .getByRole('listitem')
    .first();
  await expect(row).toContainText('Monday report');
  await expect(row).toContainText(
    'Every Monday at 09:00. Prepares a draft for you to boss@elsewhere.example.',
  );
  await expect(row).toContainText(/Next: .*Nov 23/);
  await page.screenshot({ path: 'test-results/screens/routines.png' });

  await row.getByRole('button', { name: 'Write now' }).click();
  const composer = page.getByRole('region', { name: 'New message' });
  await expect(composer.getByLabel('Subject')).toHaveValue('Report for Wednesday');
  await expect(composer.getByRole('textbox', { name: 'Message' })).toContainText(
    'Numbers for Wed, Nov 18.',
  );
  await page.keyboard.press('Escape');
  await expect(page.getByRole('status').filter({ hasText: 'Draft saved.' })).toBeVisible();
  expect(await ipc<unknown[]>(page, 'list_drafts')).toHaveLength(1);

  await row.getByRole('button', { name: 'Skip the next run' }).click();
  await expect(row).toContainText('will be skipped');
  await row.getByRole('button', { name: 'Do not skip the next run' }).click();
  await expect(row).not.toContainText('will be skipped');

  const run = row.getByRole('switch', { name: /^Run / });
  await run.click();
  await expect(run).not.toBeChecked();
  await expect(row).toContainText('Paused');
  await row.getByRole('button', { name: 'Delete routine' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'No routines yet' })).toBeVisible();
  await clearDrafts(page);
  expect(errors).toEqual([]);
});

test('a newsletter can be left from the message: one click, hidden afterwards, remembered', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  const dispatch = await personId(page, 'Weekly Dispatch');
  const convo = (
    await ipc<{ id: number }[]>(page, 'list_conversations', { personId: dispatch })
  )[0];
  if (!convo) throw new Error('the demo has no newsletter');
  await page.goto(url(`/people/${dispatch}/c/${convo.id}`));
  await expect(page.getByRole('heading', { level: 2, name: /Issue 212/ })).toBeVisible();

  await page.getByRole('button', { name: 'Unsubscribe', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Unsubscribe from The Weekly Dispatch?' });
  await expect(
    dialog.getByText(/asks weeklydispatch\.example to take you off the list/),
  ).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/unsubscribe.png' });
  await dialog.getByRole('button', { name: 'Unsubscribe' }).click();
  await expect(page.getByText('Unsubscribed from The Weekly Dispatch.')).toBeVisible();
  await expect(page.getByRole('status').filter({ hasText: /hidden\./ })).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).last().click();
  await page.goto(url(`/people/${dispatch}/c/${convo.id}`));
  await expect(page.getByText(/You unsubscribed from The Weekly Dispatch on/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Unsubscribe', exact: true })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('Write is always at hand: the sidebar, a person, Mail, and a message you are reading', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 });
  await page.goto(url('/today'));
  const composer = page.getByRole('region', { name: 'New message' });

  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Write' })
    .click();
  await expect(composer).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(composer).toBeHidden();

  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await page.getByRole('button', { name: /^Write to Maya/ }).click();
  await expect(composer).toBeVisible();
  await expect(composer.getByTitle('maya.okafor@northwind.example')).toBeVisible();
  await page.keyboard.press('Escape');

  await page.goto(url('/river'));
  await expect(page.getByRole('button', { name: 'Write', exact: true }).first()).toBeVisible();
  await page.getByRole('list', { name: 'Conversations' }).getByRole('button').first().click();
  await page.getByRole('button', { name: 'Reply, reply to all or forward' }).click();
  await page.getByRole('menuitem', { name: 'Write a new message' }).click();
  await expect(composer).toBeVisible();
  await expect(composer.getByLabel('Subject')).toHaveValue('');
});

test('the recipient suggestions are readable in the dark theme, and the person header has room', async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'zsm-ui',
      JSON.stringify({ state: { theme: 'dark', categoriesFirst: false }, version: 0 }),
    ),
  );
  await page.setViewportSize({ width: 1500, height: 900 });
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await page.waitForTimeout(400);
  await page.screenshot({ path: 'test-results/screens/person-header-dark.png' });

  const write = await page.getByRole('button', { name: /^Write to Maya/ }).boundingBox();
  const mark = page
    .getByRole('region', { name: 'Conversations' })
    .getByRole('button', { name: /unread as read/ });
  const tabs = await page.getByRole('tab', { name: /^Conversations/ }).boundingBox();
  if (write && tabs) expect(tabs.y - (write.y + write.height)).toBeGreaterThanOrEqual(12);
  if ((await mark.count()) > 0 && write) {
    const m = await mark.boundingBox();
    if (m && Math.abs(m.y - write.y) < write.height)
      expect(m.x - (write.x + write.width)).toBeGreaterThanOrEqual(12);
  }

  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Write' })
    .click();
  const composer = page.getByRole('region', { name: 'New message' });
  await expect(composer).toBeVisible();
  await page.waitForTimeout(300);
  await page.keyboard.type('maya');
  const list = composer.getByRole('listbox');
  await expect(list).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/composer-suggestions-dark.png' });
  const results = await new AxeBuilder({ page })
    .include('[role="listbox"]')
    .withRules(['color-contrast'])
    .analyze();
  expect(results.violations.map((v) => v.id)).toEqual([]);
});
