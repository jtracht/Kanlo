const $ = (id) => document.getElementById(id);
let activeBoard;
let loading = false;
async function api(path, method = 'GET', body) {
  const response = await fetch(`/api/v1${path}`, {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    const data = await response.json().catch(() => null);
    throw new Error(data?.error?.message || `Anfrage fehlgeschlagen (${response.status})`);
  }
  return response.json();
}
function element(tag, text, className) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}
function report(error) { $('status').textContent = error.message; }
async function loadBoard() {
  loading = true;
  $('boards').disabled = true;
  $('create').querySelector('button').disabled = true;
  $('status').textContent = 'Board wird geladen …';
  try {
    activeBoard = $('boards').value;
    const [columns, tickets] = await Promise.all([
      api(`/boards/${activeBoard}/columns`), api(`/boards/${activeBoard}/tickets`),
    ]);
    $('title').textContent = $('boards').selectedOptions[0].textContent;
    $('board').replaceChildren();
    const destinations = [{ id: null, name: 'Backlog' }, ...columns];
    for (const column of destinations) {
      const section = element('section', undefined, 'column');
      const items = tickets.filter((ticket) => ticket.column_id === column.id);
      section.append(element('h2', `${column.name} · ${items.length}`));
      if (column.wip_limit !== undefined && column.wip_limit !== null) {
        section.append(element('p', `WIP ${items.length}/${column.wip_limit} · ${column.wip_strict ? 'Sperre' : 'Warnung'}`, 'wip'));
      }
      for (const ticket of items) {
        const card = element('article', undefined, 'ticket');
        card.append(element('span', ticket.public_id, 'ticket-id'), element('h3', ticket.title));
        if (ticket.description) card.append(element('p', ticket.description));
        const label = element('label', 'Verschieben nach');
        const select = element('select');
        select.setAttribute('aria-label', `${ticket.public_id} verschieben nach`);
        for (const destination of destinations) {
          const option = element('option', destination.name);
          option.value = destination.id || '';
          option.selected = destination.id === ticket.column_id;
          select.append(option);
        }
        select.addEventListener('change', async () => {
          select.disabled = true;
          try {
            await api(`/tickets/${ticket.id}/move`, 'POST', { board_id: activeBoard, column_id: select.value || null, base_revision: ticket.revision });
            await loadBoard();
          } catch (error) {
            select.value = ticket.column_id || '';
            select.disabled = false;
            report(error);
          }
        });
        label.append(select);
        card.append(label);
        section.append(card);
      }
      if (!items.length) section.append(element('p', 'Noch keine Tickets', 'empty'));
      $('board').append(section);
    }
    $('status').textContent = `${tickets.length} Tickets · Änderungen werden auf diesem Gerät gespeichert.`;
  } finally {
    loading = false;
    $('boards').disabled = false;
    $('create').querySelector('button').disabled = false;
  }
}
$('boards').addEventListener('change', () => loadBoard().catch(report));
$('create').addEventListener('submit', async (event) => {
  event.preventDefault();
  if (loading || !activeBoard) return;
  const button = $('create').querySelector('button');
  button.disabled = true;
  try {
    await api(`/boards/${activeBoard}/tickets`, 'POST', { title: $('new-title').value });
    $('new-title').value = '';
    await loadBoard();
  } catch (error) { report(error); }
  finally { button.disabled = false; }
});
async function start() {
  const boards = await api('/boards');
  for (const board of boards) {
    const option = element('option', board.name);
    option.value = board.id;
    $('boards').append(option);
  }
  await loadBoard();
}
start().catch(report);
