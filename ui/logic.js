// Pure list logic, shared by index.html. Self-check: node ui/logic.js

const dayKey = (d = new Date()) => d.toLocaleDateString('sv'); // local YYYY-MM-DD

// New day: drop finished items, send unfinished "today" items back to "later".
function rollover(state, day) {
  if (state.day === day) return false;
  state.items = state.items.filter((i) => !i.done);
  state.items.forEach((i) => (i.list = 'later'));
  state.day = day;
  return true;
}

// Move item `id` into `list`, before `targetId` (null = end).
function moveBefore(items, id, targetId, list) {
  const from = items.findIndex((i) => i.id === id);
  if (from < 0 || id === targetId) return;
  const [item] = items.splice(from, 1);
  item.list = list;
  const to = targetId == null ? -1 : items.findIndex((i) => i.id === targetId);
  if (to < 0) items.push(item);
  else items.splice(to, 0, item);
}

if (typeof module !== 'undefined' && require.main === module) {
  const assert = require('assert');
  const mk = () => [
    { id: 'a', list: 'today', done: true },
    { id: 'b', list: 'today', done: false },
    { id: 'c', list: 'later', done: false },
  ];
  const s = { day: '2026-01-01', items: mk() };
  assert.equal(rollover(s, '2026-01-01'), false);
  assert.equal(s.items.length, 3);
  assert.equal(rollover(s, '2026-01-02'), true);
  assert.deepEqual(s.items.map((i) => i.id + i.list), ['blater', 'clater']);

  const items = mk();
  moveBefore(items, 'c', 'a', 'today');
  assert.deepEqual(items.map((i) => i.id + i.list), ['ctoday', 'atoday', 'btoday']);
  moveBefore(items, 'c', null, 'later');
  assert.deepEqual(items.map((i) => i.id + i.list), ['atoday', 'btoday', 'clater']);
  moveBefore(items, 'a', 'a', 'later'); // dropped on itself: no-op
  assert.equal(items[0].list, 'today');
  assert.match(dayKey(new Date(2026, 0, 5)), /^2026-01-05$/);
  console.log('logic ok');
}
