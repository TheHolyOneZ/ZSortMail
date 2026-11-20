import { expect, type Page, test } from '@playwright/test';

test.describe.configure({ mode: 'serial' });

const NOW = '2026-11-18T10:00:00+01:00';
const url = (hash: string) => `/?now=${encodeURIComponent(NOW)}#${hash}`;

test.use({ viewport: { width: 1360, height: 860 } });

function watchForErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('Failed to load resource'))
      errors.push(`console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  return errors;
}

test('the wall shows everyone as a stamp, opens a person, moves with the arrow keys, and goes back to the list', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  const people = page.getByRole('listbox', { name: 'People' });
  await expect(people.getByRole('option').first()).toBeVisible();
  const inList = await people.getByRole('option').count();

  await page.getByRole('radio', { name: 'Wall' }).click();
  const wall = page.getByRole('listbox', { name: 'People' });
  await expect(wall.getByRole('option', { name: /Maya Okafor/ })).toBeVisible();
  await expect(wall.getByRole('option')).toHaveCount(23);
  expect(inList).toBeGreaterThan(5);
  await page.waitForTimeout(700);
  await page.screenshot({ path: 'test-results/screens/wall.png' });

  await wall.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page).toHaveURL(/#\/people\/\d+/);
  await expect(page.getByRole('heading', { level: 2, name: 'Maya Okafor' })).toBeVisible();
  await expect(wall.getByRole('option', { name: /Maya Okafor/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );

  await wall.focus();
  await page.keyboard.press('ArrowRight');
  await expect(wall.getByRole('option', { name: /Maya Okafor/ })).toHaveAttribute(
    'aria-selected',
    'false',
  );
  await expect(wall.locator('[aria-selected="true"]')).toHaveCount(1);

  await page.reload();
  await expect(page.getByRole('radio', { name: 'Wall' })).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('radio', { name: 'List' }).click();
  await expect(
    page.getByRole('listbox', { name: 'People' }).getByRole('option').first(),
  ).toBeVisible();
  expect(errors).toEqual([]);
});

test('a message shows who it is from, to and copied, and Mark all read cleans the badges with one Undo', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();
  await expect(page.getByRole('list', { name: 'Email addresses' })).toContainText(
    'maya.okafor@northwind.example',
  );

  const strand = page.getByRole('region', { name: 'Conversation', exact: true });
  await expect(strand.getByText('maya.okafor@northwind.example').first()).toBeVisible();
  await strand.getByRole('button', { name: 'Details' }).first().click();
  await expect(strand.getByText('From', { exact: true }).first()).toBeVisible();
  await expect(strand.getByText('To', { exact: true }).first()).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/message-details.png' });

  await page
    .getByRole('button', { name: /Q3 launch checklist/ })
    .first()
    .click({ button: 'right' });
  const menu = page.getByRole('menu', { name: 'Actions' });
  await expect(menu.getByRole('menuitem', { name: 'Reply' })).toBeVisible();
  await expect(menu.getByRole('menuitem', { name: 'Archive' })).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/context-menu.png' });
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();

  await page.getByRole('option', { name: /Dad/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Edit profile' }).click();
  await expect(page.getByRole('dialog', { name: /Edit Dad/ })).toBeVisible();
  await page.keyboard.press('Escape');

  await page.goto(url('/people'));
  const peoplePane = page.getByRole('region', { name: 'People', exact: true });
  const all = peoplePane.getByRole('button', { name: /^Mark all \d+ unread as read/ });
  await expect(all).toBeVisible();
  const label = (await all.textContent()) ?? '';
  await all.click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Marked everything read.' }),
  ).toBeVisible();
  await expect(all).toHaveCount(0);
  await page.getByRole('button', { name: 'Undo' }).last().click();
  await expect(page.getByRole('button', { name: /^Mark all \d+ unread as read/ })).toHaveText(
    label,
  );
  expect(errors).toEqual([]);
});

test('the sidebar collapses to icons and opens to names, and the choice is remembered', async ({
  page,
}) => {
  await page.goto(url('/people'));
  const nav = page.getByRole('navigation', { name: 'Main' });
  await expect(nav.getByText('New senders', { exact: true })).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/sidebar-open.png' });
  await nav.getByRole('button', { name: 'Collapse the sidebar' }).click();
  await expect(nav.getByText('New senders', { exact: true })).toHaveCount(0);
  await expect(nav.getByRole('link', { name: 'New senders' })).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/sidebar-collapsed.png' });
  await page.reload();
  await expect(nav.getByRole('link', { name: 'New senders' })).toBeVisible();
  await expect(nav.getByText('New senders', { exact: true })).toHaveCount(0);
  await page.keyboard.press('Control+b');
  await expect(nav.getByText('New senders', { exact: true })).toBeVisible();
});

test('dialogs share one frame: a title, a way to close, a scrolling body and a footer that stays in view', async ({
  page,
}) => {
  await page.goto(url('/routines'));
  await page.getByRole('button', { name: 'New routine' }).click();
  const dialog = page.getByRole('dialog', { name: 'New routine' });
  await expect(dialog.getByRole('heading', { name: 'New routine' })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Close' })).toBeVisible();
  const save = dialog.getByRole('button', { name: 'Save routine' });
  await expect(save).toBeInViewport();
  await page.screenshot({ path: 'test-results/screens/modal-routine.png' });
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(dialog).toBeHidden();

  await page.goto(url('/waiting'));
  await page.getByRole('button', { name: 'Watch for an email' }).click();
  await page.screenshot({ path: 'test-results/screens/modal-watch.png' });
  await page.keyboard.press('Escape');
});

test('Mail: a search narrows the list, can be kept as a view, and many conversations change with one Undo', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/river'));
  const lines = page.getByRole('list', { name: 'Conversations' }).getByRole('listitem');
  await expect(lines.first()).toBeVisible();
  const all = await lines.count();

  await page.getByRole('searchbox', { name: /Search your mail/ }).fill('category:newsletter');
  await expect(lines).toHaveCount(1);
  await expect(lines.first()).toContainText('The Weekly Dispatch');
  await page.getByRole('button', { name: 'Save as a view' }).click();
  const dialog = page.getByRole('dialog', { name: 'Save as a view' });
  await dialog.getByLabel('Name').fill('Newsletters only');
  await dialog.getByRole('button', { name: 'Save view' }).click();
  const view = page.getByRole('button', { name: 'Newsletters only', exact: true });
  await expect(view).toBeVisible();

  await page.getByRole('searchbox', { name: /Search your mail/ }).fill('');
  await expect(lines).toHaveCount(all);
  await view.click();
  await expect(lines).toHaveCount(1);
  await page.getByRole('button', { name: 'Remove the view Newsletters only' }).click();
  await expect(view).toHaveCount(0);
  await page.getByRole('searchbox', { name: /Search your mail/ }).fill('');

  await expect(lines).toHaveCount(all);
  for (const n of [0, 1, 2]) await lines.nth(n).getByRole('checkbox').check();
  const bar = page.getByRole('toolbar', { name: 'Selected conversations' });
  await expect(page.getByText('3 conversations selected')).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/bulk.png' });
  await bar.getByRole('button', { name: 'Archive' }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Archived 3 conversations.' }),
  ).toBeVisible();
  await expect(lines).toHaveCount(all - 3);
  await page.getByRole('button', { name: 'Undo' }).last().click();
  await expect(lines).toHaveCount(all);
  expect(errors).toEqual([]);
});

test('the person timeline interleaves every conversation, and j and k step through conversations', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();
  await page.getByRole('tab', { name: 'Timeline' }).click();
  const panel = page.getByRole('tabpanel');
  await expect(panel.getByRole('region', { name: 'Today' })).toBeVisible();
  await expect(panel.getByText('Coffee Thursday?').first()).toBeVisible();
  await expect(panel.getByText('Invoice 2291').first()).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/timeline.png' });

  await page.getByRole('tab', { name: 'Conversations' }).click();
  await page
    .getByRole('region', { name: 'Conversation', exact: true })
    .click({ position: { x: 5, y: 5 } });
  const heading = page.getByRole('heading', { level: 2 }).last();
  await expect(heading).toHaveText('Q3 launch checklist');
  await page.keyboard.press('j');
  await expect(page.getByRole('heading', { level: 2, name: 'Coffee Thursday?' })).toBeVisible();
  await page.keyboard.press('k');
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();
  expect(errors).toEqual([]);
});

test('dividers can be dragged or moved with the arrow keys, double-click resets them, and sizes are remembered', async ({
  page,
}) => {
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();

  const people = page.getByRole('separator', { name: 'Resize the People list' });
  const before = Number(await people.getAttribute('aria-valuenow'));
  const box = await people.boundingBox();
  if (!box) throw new Error('no divider');
  await page.mouse.move(box.x + box.width / 2, box.y + 200);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 60, box.y + 200, { steps: 6 });
  await page.mouse.up();
  const after = Number(await people.getAttribute('aria-valuenow'));
  expect(after).toBeGreaterThan(before + 40);

  await people.focus();
  await page.keyboard.press('ArrowLeft');
  expect(Number(await people.getAttribute('aria-valuenow'))).toBe(after - 16);
  await page.keyboard.press('Home');
  expect(Number(await people.getAttribute('aria-valuenow'))).toBe(
    Number(await people.getAttribute('aria-valuemin')),
  );
  await page.keyboard.press('Enter');
  expect(Number(await people.getAttribute('aria-valuenow'))).toBe(before);

  const rail = page.getByRole('separator', { name: 'Resize the sidebar' });
  await rail.focus();
  await page.keyboard.press('Shift+ArrowRight');
  const wider = Number(await rail.getAttribute('aria-valuenow'));
  await page.reload();
  await expect(page.getByRole('separator', { name: 'Resize the sidebar' })).toHaveAttribute(
    'aria-valuenow',
    String(wider),
  );
  await page.getByRole('separator', { name: 'Resize the sidebar' }).dblclick();
  await expect(page.getByRole('separator', { name: 'Resize the sidebar' })).toHaveAttribute(
    'aria-valuenow',
    '212',
  );
});

test('tags go on a conversation, find it from the Mail screen, and come off; two people merge and split with Undo', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();

  await page.getByRole('button', { name: 'Tag', exact: true }).click();
  await page.getByRole('textbox', { name: 'New tag' }).fill('Launch');
  await page.keyboard.press('Enter');
  const chip = page.getByRole('button', { name: 'Launch', exact: true });
  await expect(chip).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/tags.png' });

  await chip.click();
  await expect(page).toHaveURL(/#\/river/);
  const lines = page.getByRole('list', { name: 'Conversations' }).getByRole('listitem');
  await expect(lines).toHaveCount(1);
  await expect(lines.first()).toContainText('Q3 launch checklist');
  await page.getByRole('searchbox', { name: /Search your mail/ }).fill('');

  await page.goBack();
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();
  await page.getByRole('button', { name: 'Remove the tag Launch' }).click();
  await expect(page.getByRole('button', { name: 'Launch', exact: true })).toHaveCount(0);

  await page.getByRole('option', { name: /Dad/ }).click();
  await page.getByRole('button', { name: 'More about this person' }).click();
  await page.getByRole('menuitem', { name: 'Merge into another person…' }).click();
  const dialog = page.getByRole('dialog', { name: /Merge Dad/ });
  await dialog.getByRole('radio', { name: /Maya Okafor/ }).check();
  await page.screenshot({ path: 'test-results/screens/merge.png' });
  await dialog.getByRole('button', { name: 'Merge into Maya Okafor' }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Merged Dad into Maya Okafor.' }),
  ).toBeVisible();
  await expect(page.getByRole('option', { name: /^Dad/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'Undo' }).last().click();
  await expect(page.getByRole('option', { name: /^Dad/ })).toBeVisible();
  expect(errors).toEqual([]);
});

test('an address book file fills in roles for people you already write with', async ({ page }) => {
  await page.goto(url('/settings'));
  await page.getByLabel('Choose a vCard file').setInputFiles({
    name: 'contacts.vcf',
    mimeType: 'text/vcard',
    buffer: Buffer.from(
      'BEGIN:VCARD\nVERSION:3.0\nFN:Tomás Herrera\nEMAIL:tomas@cicloshop.example\nORG:Ciclo Shop\nTITLE:Mechanic\nEND:VCARD\nBEGIN:VCARD\nFN:Stranger\nEMAIL:stranger@elsewhere.example\nEND:VCARD\n',
    ),
  });
  await expect(page.getByRole('status').filter({ hasText: /Read 2 contacts/ })).toBeVisible();
  await expect(
    page.getByRole('status').filter({ hasText: /1 contact had no matching person/ }),
  ).toBeVisible();
  await page.getByRole('link', { name: 'People' }).click();
  await page.getByRole('option', { name: /Tomás Herrera/ }).click();
  await expect(page.getByText('Bike shop, Ciclo Shop')).toBeVisible();
});

test('Mail: select all, shift-click for a range, big targets, and quick actions on every line', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/river'));
  const lines = page.getByRole('list', { name: 'Conversations' }).getByRole('listitem');
  await expect(lines.first()).toBeVisible();
  const total = await lines.count();

  const box = await lines.first().getByRole('checkbox').locator('xpath=..').boundingBox();
  expect(box?.width).toBeGreaterThanOrEqual(40);
  const archive = await lines.first().getByRole('button', { name: 'Archive' }).boundingBox();
  expect(archive?.width).toBeGreaterThanOrEqual(40);
  const snooze = await lines.first().getByRole('button', { name: 'Snooze' }).boundingBox();
  expect(snooze?.width).toBeGreaterThanOrEqual(40);

  const all = page.getByRole('checkbox', { name: 'Select all conversations' });
  await all.check();
  await expect(page.getByRole('toolbar', { name: 'Selected conversations' })).toBeVisible();
  await expect(page.getByText(`${total} conversations selected`)).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/mail-select-all.png' });
  await all.uncheck();
  await expect(page.getByRole('toolbar', { name: 'Selected conversations' })).toHaveCount(0);

  await lines.nth(1).getByRole('checkbox').click();
  await lines
    .nth(4)
    .getByRole('checkbox')
    .click({ modifiers: ['Shift'] });
  await expect(page.getByText('4 conversations selected')).toBeVisible();
  expect(await all.evaluate((el: HTMLInputElement) => el.indeterminate)).toBe(true);

  await page.getByRole('button', { name: 'Clear' }).click();
  await lines.first().getByRole('button', { name: 'Archive' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Archived.' })).toBeVisible();
  await expect(lines).toHaveCount(total - 1);
  await page.getByRole('button', { name: 'Undo' }).last().click();
  await expect(lines).toHaveCount(total);
  expect(errors).toEqual([]);
});

test('People opens on categories: make one, add someone under their own name, see favourites everywhere', async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem('zsm-ui', JSON.stringify({ state: {}, version: 0 })),
  );
  await page.goto(url('/people'));
  const categories = page.getByRole('list', { name: 'Categories' });
  await expect(categories.getByRole('button', { name: /^Everyone/ })).toBeVisible();
  await expect(categories.getByRole('button', { name: /^Favourites/ })).toBeVisible();

  await categories.getByRole('button', { name: 'New category' }).click();
  const dialog = page.getByRole('dialog', { name: 'New category' });
  await dialog.getByLabel('Name', { exact: true }).fill('Doctors');
  await dialog.getByLabel('Description').fill('People who look after me');
  await dialog.getByTitle('Health').click();
  await dialog.getByTitle('Colour 6').click();
  await dialog.getByRole('button', { name: 'Save and add people' }).click();
  await dialog.getByLabel('Find someone you already write to').fill('Hanna');
  await dialog.getByRole('list', { name: 'People found' }).getByRole('button').first().click();
  await dialog.getByRole('textbox', { name: /Name in this category for/ }).fill('Dr. Berg');
  await dialog.getByRole('textbox', { name: /Name in this category for/ }).blur();
  await dialog.getByLabel('Or add an email address').fill('dr.aydin@clinic.example');
  await dialog.getByLabel('Name to show').fill('Dr. Aydin');
  await dialog.getByRole('button', { name: 'Add', exact: true }).click();
  await expect(dialog.getByRole('list', { name: 'Members' }).getByRole('listitem')).toHaveCount(2);
  await dialog.getByRole('button', { name: 'Save category' }).click();
  await page.screenshot({ path: 'test-results/screens/people-categories.png' });

  await categories.getByRole('button', { name: /^Doctors/ }).click();
  const list = page.getByRole('region', { name: 'People' });
  await expect(list.getByText('Dr. Berg')).toBeVisible();
  await expect(list.getByText('Dr. Aydin')).toBeVisible();
  await expect(list.getByRole('option')).toHaveCount(2);
  await page.getByRole('button', { name: 'All categories' }).click();

  await categories.getByRole('button', { name: /^Everyone/ }).click();
  await page
    .getByRole('option', { name: /Okafor/ })
    .first()
    .click({ button: 'right' });
  await page.getByRole('menuitem', { name: /^(Add to|Remove from) favourites$/ }).click();
  await expect(page.locator('[data-favourite="true"]').first()).toBeVisible();
  await page.getByRole('button', { name: 'All categories' }).click();
  await categories.getByRole('button', { name: /^Favourites/ }).click();
  await expect(
    page.getByRole('region', { name: 'People' }).getByRole('option', { name: /Okafor/ }),
  ).toBeVisible();
});

test('the sidebar: tabs are hidden, shown and reordered in Settings, and a category can be pinned there', async ({
  page,
}) => {
  await page.goto(url('/settings'));
  const nav = page.getByRole('navigation', { name: 'Main' });
  await expect(nav.getByRole('link', { name: 'Routines' })).toBeVisible();

  await page.getByRole('switch', { name: 'Show Routines in the sidebar' }).click();
  await expect(nav.getByRole('link', { name: 'Routines' })).toHaveCount(0);

  const names = async () =>
    nav.getByRole('link').evaluateAll((a) => a.map((x) => x.getAttribute('aria-label')));
  await expect.poll(async () => (await names())[0]).toBe('Today');
  await page.getByRole('button', { name: 'Move Today down' }).click();
  await expect.poll(async () => (await names()).slice(0, 2)).toEqual(['People', 'Today']);

  await page.reload();
  await expect.poll(async () => (await names()).slice(0, 2)).toEqual(['People', 'Today']);
  await expect(nav.getByRole('link', { name: 'Routines' })).toHaveCount(0);

  await page.getByRole('button', { name: 'Reset the sidebar' }).click();
  await expect.poll(async () => (await names())[0]).toBe('Today');
  await expect(nav.getByRole('link', { name: 'Routines' })).toBeVisible();

  await page.goto(url('/people'));
  await page.getByRole('button', { name: 'All categories' }).click();
  await page.getByRole('button', { name: 'New category' }).click();
  const dialog = page.getByRole('dialog', { name: 'New category' });
  await dialog.getByLabel('Name', { exact: true }).fill('Pinned crew');
  await dialog.getByRole('switch', { name: 'Show this category in the sidebar' }).click();
  await dialog.getByRole('button', { name: 'Save category' }).click();
  await page.goto(url('/today'));
  await nav.getByRole('link', { name: 'Category Pinned crew' }).click();
  await expect(page.getByRole('heading', { name: /Pinned crew/ })).toBeVisible();
});

test('Mail looks like a mail site: mailboxes on the left, the list, and the message beside it', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 });
  await page.goto(url('/river'));
  const boxes = page.getByRole('navigation', { name: 'Mailboxes' });
  await expect(boxes.getByRole('button', { name: /^Inbox/ })).toHaveAttribute(
    'aria-current',
    'true',
  );

  const list = page.getByRole('list', { name: 'Conversations' });
  await list.getByRole('button').first().click();
  await expect(page.getByRole('region', { name: 'Conversation' })).toBeVisible();
  await expect(list.getByRole('listitem').first()).toHaveAttribute('aria-current', 'true');
  expect(page.url()).toContain('/river');

  await boxes.getByRole('button', { name: 'Sent' }).click();
  await expect(boxes.getByRole('button', { name: 'Sent' })).toHaveAttribute('aria-current', 'true');
  await expect(
    page
      .getByRole('tab', { name: 'Newsletters' })
      .or(page.getByRole('button', { name: /Newsletters/ })),
  ).toHaveCount(0);
  await page.screenshot({ path: 'test-results/screens/mail-gmail-style.png' });

  await page.setViewportSize({ width: 900, height: 800 });
  await boxes.getByRole('button', { name: /^Inbox/ }).click();
  await expect(page.getByRole('region', { name: 'Conversation' })).toHaveCount(0);
});

test('profiles: a mailbox has a profile, and the sharing switch is there', async ({ page }) => {
  await page.goto(url('/settings'));
  const profiles = page.getByRole('list', { name: 'Profiles' });
  await expect(profiles.getByRole('listitem')).toHaveCount(1);
  await expect(profiles.getByText('In use')).toBeVisible();
  const share = page.getByRole('switch', { name: 'Keep settings the same on every profile' });
  await expect(share).not.toBeChecked();
  await share.click();
  await expect(share).toBeChecked();
  await share.click();
  await expect(page.getByRole('combobox', { name: 'Profile' })).toHaveCount(0);
});

test('a person with two addresses can have one split off, and Undo joins them again', async ({
  page,
}) => {
  const find = async (q: string) => {
    const r = await page.request.post('/ipc/list_people', { data: { filter: 'all', query: q } });
    return ((await r.json()) as { id: number; name: string }[])[0] as { id: number; name: string };
  };
  const keep = await find('Chloé');
  const other = await find('Noa');
  await page.request.post('/ipc/merge_people', { data: { keep: keep.id, absorb: other.id } });

  await page.goto(url(`/people/${keep.id}`));
  await page.getByRole('option', { name: /Chloé/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Edit profile' }).click();
  const dialog = page.getByRole('dialog', { name: /Edit Chloé/ });
  const split = dialog.getByRole('button', { name: /^Split .* into its own person$/ });
  await expect(split).toHaveCount(2);
  await split.last().click();
  await expect(page.getByText(/^Split .* off Chloé/)).toBeVisible();

  const after = await page.request.post('/ipc/list_people', { data: { filter: 'all', query: '' } });
  const names = ((await after.json()) as { id: number }[]).length;
  await page.getByRole('button', { name: 'Undo' }).last().click();
  await expect
    .poll(async () => {
      const r = await page.request.post('/ipc/list_people', { data: { filter: 'all', query: '' } });
      return ((await r.json()) as unknown[]).length;
    })
    .toBe(names - 1);
  await page.keyboard.press('Control+z');
});

test('inside a category you can add people and email addresses, and add anyone to a category from their menu', async ({
  page,
}) => {
  await page.goto(url('/people'));
  await page.getByRole('button', { name: 'All categories' }).click();
  await page.getByRole('button', { name: 'New category' }).click();
  const make = page.getByRole('dialog', { name: 'New category' });
  await make.getByLabel('Name', { exact: true }).fill('Neighbours');
  await make.getByRole('button', { name: 'Save category' }).click();
  await page
    .getByRole('list', { name: 'Categories' })
    .getByRole('button', { name: /^Neighbours/ })
    .click();

  await expect(page.getByText('Nobody is in this category yet.')).toBeVisible();
  await page.getByRole('button', { name: 'Add people' }).first().click();
  const add = page.getByRole('dialog', { name: 'People in Neighbours' });
  await add.getByLabel('Or add an email address').fill('carla@street.example');
  await add.getByLabel('Name to show').fill('Carla next door');
  await add.getByRole('button', { name: 'Add', exact: true }).click();
  await add.getByLabel('Find someone you already write to').fill('Dad');
  await add.getByRole('list', { name: 'People found' }).getByRole('button').first().click();
  await expect(add.getByRole('list', { name: 'Members' }).getByRole('listitem')).toHaveCount(2);
  await add.getByRole('button', { name: 'Done' }).click();
  const list = page.getByRole('region', { name: 'People' });
  await expect(list.getByRole('option', { name: /Carla next door/ })).toBeVisible();
  await expect(list.getByRole('option', { name: /Dad/ })).toBeVisible();

  await page.getByRole('button', { name: 'All categories' }).click();
  await page
    .getByRole('list', { name: 'Categories' })
    .getByRole('button', { name: /^Everyone/ })
    .click();
  await page.getByRole('option', { name: /Maya Okafor/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Add to a category' }).click();
  const pick = page.getByRole('dialog', { name: /Categories for Maya/ });
  await pick.getByText('Neighbours', { exact: true }).click();
  await expect(pick.getByRole('checkbox', { name: /Neighbours/ })).toBeChecked();
  await pick.getByRole('button', { name: 'Done' }).click();
  await page.getByRole('button', { name: 'All categories' }).click();
  await page
    .getByRole('list', { name: 'Categories' })
    .getByRole('button', { name: /^Neighbours/ })
    .click();
  await expect(page.getByRole('region', { name: 'People' }).getByRole('option')).toHaveCount(3);
});

test('a second mailbox gets a profile: the setup question, the switcher, and lists that follow the profile', async ({
  page,
}) => {
  try {
    await second(page);
  } finally {
    const state = await (await page.request.post('/ipc/app_state', { data: {} })).json();
    for (const a of state.accounts as { id: number; email: string }[])
      if (a.email === 'work@demo.example')
        await page.request.post('/ipc/unlink_account', { data: { id: a.id } });
    await page.request.post('/ipc/set_active_account', { data: { account: null } });
  }
});

async function second(page: Page) {
  const theme = () =>
    page.evaluate(() => JSON.parse(localStorage.getItem('zsm-ui') ?? '{}').state?.theme as string);
  await page.goto(url('/settings'));
  await page.getByRole('radio', { name: /^Ink/ }).check({ force: true });
  await expect.poll(theme).toBe('dark');

  await page.request.post('/ipc/start_second_demo', { data: {} });
  const ask = page.getByRole('dialog', { name: 'New profile' });
  await expect(ask).toBeVisible();
  await expect(ask.getByText('Set up work@demo.example')).toBeVisible();
  await ask.getByRole('button', { name: 'Start fresh' }).click();

  await expect.poll(theme).toBe('system');
  const nav = page.getByRole('navigation', { name: 'Main' });
  const switcher = nav.getByRole('combobox', { name: 'Profile' });
  await expect(switcher).toBeVisible();

  await page.goto(url('/people'));
  await page
    .getByRole('list', { name: 'Categories' })
    .getByRole('button', { name: /^Everyone/ })
    .click();
  const people = page.getByRole('listbox', { name: 'People' });
  await expect(people.getByRole('option', { name: /Wanda/ })).toBeVisible();
  await expect(people.getByRole('option')).toHaveCount(2);

  await switcher.click();
  await page.getByRole('option', { name: 'Demo mailbox' }).click();
  await expect.poll(theme).toBe('dark');
  await expect(people.getByRole('option', { name: /Maya Okafor/ })).toBeVisible();
  await expect(people.getByRole('option', { name: /Wanda/ })).toHaveCount(0);

  await page.goto(url('/settings'));
  await page.getByRole('switch', { name: 'Keep settings the same on every profile' }).click();
  await nav.getByRole('combobox', { name: 'Profile' }).click();
  await page.getByRole('option', { name: 'Work mailbox' }).click();
  await expect.poll(theme).toBe('dark');
}

test('in German the new screens speak German: mailboxes, categories, filters and the bulk bar', async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'zsm-ui',
      JSON.stringify({ state: { language: 'de', categoriesFirst: true }, version: 0 }),
    ),
  );
  await page.goto(url('/people'));
  const tiles = page.getByRole('list', { name: 'Categories' });
  await expect(tiles.getByRole('button', { name: /^Alle/ })).toBeVisible();
  await expect(tiles.getByRole('button', { name: /^Favoriten/ })).toBeVisible();
  await tiles.getByRole('button', { name: /^Alle/ }).click();
  await expect(
    page.getByRole('button', { name: 'Braucht Antwort', exact: false }).first(),
  ).toBeVisible();

  await page.goto(url('/river'));
  const boxes = page.getByRole('navigation', { name: 'Mailboxes' });
  await expect(boxes.getByRole('button', { name: /^Posteingang/ })).toBeVisible();
  await expect(boxes.getByRole('button', { name: 'Papierkorb' })).toBeVisible();
  await page.getByRole('checkbox', { name: 'Select all conversations' }).check({ force: true });
  await expect(page.getByRole('button', { name: 'Archivieren' }).first()).toBeVisible();
});

test('Google setup is a guided tutorial with links, copy buttons and a way to paste the downloaded file', async ({
  page,
}) => {
  await page.goto(url('/settings'));
  await page.getByRole('button', { name: 'Set up Google access' }).first().click();
  const guide = page.getByRole('dialog', { name: 'Set up Google access' });
  await expect(guide.getByText('Step 1 of 7')).toBeVisible();
  await expect(guide.getByRole('link', { name: /Open “New project”/ })).toHaveAttribute(
    'href',
    'https://console.cloud.google.com/projectcreate',
  );
  await page.waitForTimeout(500);
  await page.screenshot({ path: 'test-results/screens/google-guide-1.png' });

  for (let i = 0; i < 4; i++) await guide.getByRole('button', { name: 'Done, next step' }).click();
  await expect(guide.getByText('5. Allow the Gmail permission')).toBeVisible();
  await expect(guide.getByRole('button', { name: /Copy gmail.modify/ })).toBeVisible();
  await guide.getByRole('button', { name: 'Done, next step' }).click();
  await expect(guide.getByText('Desktop app').first()).toBeVisible();
  await guide.getByRole('button', { name: 'I have the ID and secret' }).click();

  await guide.getByText('Or paste the downloaded JSON file').click();
  await guide
    .getByLabel('Downloaded JSON file')
    .fill(
      '{"installed":{"client_id":"123-abc.apps.googleusercontent.com","client_secret":"GOCSPX-test"}}',
    );
  await expect(guide.getByLabel('Client ID')).toHaveValue('123-abc.apps.googleusercontent.com');
  await guide.getByLabel('Client ID').fill('GOCSPX-oops');
  await expect(guide.getByText(/ends in .apps.googleusercontent.com/)).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/google-guide-last.png' });
  await guide.getByRole('button', { name: 'Close' }).click();
});
