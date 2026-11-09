import type {
  Bootstrap,
  Command,
  QueueEntry,
  Snapshot,
  Ticket,
  View,
  Workspace,
} from "./types";
import { defaultOptions } from "./types";

export function optimistic(
  snapshot: Snapshot,
  entry: QueueEntry,
  cached: Snapshot[],
): Snapshot {
  const result = structuredClone(snapshot);
  const state = result.workspace;
  const command = entry.action;
  const all = [
    ...new Map(
      [...state.tickets, ...cached.flatMap((item) => item.workspace.tickets)]
        .map((ticket) => [ticket.id, ticket] as const)
        .reverse(),
    ).values(),
  ];
  if (entry.state === "confirmation") return result;
  const key = String(command.id ?? "");
  const root = all.find(
    (ticket) => ticket.id === key || ticket.public_id === key,
  );
  const family = root
    ? all.filter(
        (ticket) =>
          ticket.id === root.id || ticket.details.parent_id === root.id,
      )
    : [];
  const stamp = entry.created_at;
  if (
    command.kind === "create_ticket" &&
    command.board_id === state.boards[0]?.id
  ) {
    const parent = state.tickets.find(
      (ticket) => ticket.id === command.parent_id,
    );
    const column_id = parent
      ? parent.column_id
      : ((command.column_id as string | null | undefined) ?? null);
    const done =
      parent?.done ??
      !!state.columns.find((column) => column.id === column_id)?.is_done;
    state.tickets.push({
      id: String(command.local_id),
      public_id: "Lokal",
      board_id: String(command.board_id),
      column_id,
      title: String(command.title),
      description: String(command.description ?? ""),
      importance: Number(command.importance ?? 0),
      done,
      revision: 0,
      details: {
        parent_id: (command.parent_id as string | null) ?? null,
        type_id: (command.type_id as string | null) ?? null,
        labels: [],
        custom_fields: {},
        deadline: null,
        archived: false,
        trashed: false,
        deleted_at: null,
        completed_at: done ? stamp : null,
        rank: Date.parse(stamp),
        last_active_column: column_id,
        created_at: stamp,
        updated_at: stamp,
      },
    });
  }
  if (command.kind === "patch_ticket") {
    const ticket = state.tickets.find((ticket) => ticket.id === key);
    if (ticket) {
      for (const [field, value] of Object.entries(
        command.fields as Record<string, unknown>,
      )) {
        if (field === "title" || field === "description")
          ticket[field] = String(value);
        else if (field === "importance") ticket.importance = Number(value);
        else if (field === "type_id")
          ticket.details.type_id = value as string | null;
        else if (field === "labels") ticket.details.labels = value as string[];
        else if (field === "custom_fields")
          ticket.details.custom_fields = value as Record<string, string>;
        else if (field === "deadline")
          ticket.details.deadline = value as string | null;
      }
      ticket.details.updated_at = stamp;
    }
  }
  if (command.kind === "move_ticket" && root) {
    const targetBoard = String(command.board_id);
    const targetColumn = command.column_id as string | null;
    const destination = cached
      .flatMap((item) => item.workspace.columns)
      .find((column) => column.id === targetColumn);
    state.tickets = state.tickets.filter(
      (ticket) => !family.some((member) => member.id === ticket.id),
    );
    if (state.boards[0]?.id === targetBoard)
      for (const original of family) {
        const ticket = structuredClone(original);
        ticket.board_id = targetBoard;
        ticket.column_id = targetColumn;
        if (ticket.id === root.id || destination?.is_done)
          ticket.done = !!destination?.is_done;
        if (!destination?.is_done)
          ticket.details.last_active_column = targetColumn;
        ticket.details.updated_at = stamp;
        state.tickets.push(ticket);
      }
  }
  if (command.kind === "set_done") {
    const ticket = state.tickets.find((ticket) => ticket.id === key);
    if (ticket) {
      ticket.done = !!command.done;
      if (!ticket.details.parent_id && command.done) {
        ticket.column_id =
          state.columns.find((column) => column.is_done)?.id ?? null;
        for (const child of state.tickets.filter(
          (child) => child.details.parent_id === key,
        )) {
          child.done = true;
          child.column_id = ticket.column_id;
        }
      }
      if (!ticket.details.parent_id && !command.done) {
        ticket.column_id = ticket.details.last_active_column;
        for (const child of state.tickets.filter(
          (child) => child.details.parent_id === key,
        ))
          child.column_id = ticket.column_id;
      }
    }
  }
  if (["trash", "restore", "archive"].includes(command.kind)) {
    const roots = command.ids as string[];
    for (const ticket of state.tickets)
      if (
        roots.includes(ticket.id) ||
        roots.includes(ticket.details.parent_id ?? "")
      ) {
        if (command.kind === "trash") ticket.details.trashed = true;
        if (command.kind === "restore") ticket.details.trashed = false;
        if (command.kind === "archive")
          ticket.details.archived = !!command.archived;
      }
  }
  if (
    command.kind === "add_comment" &&
    state.tickets.some((ticket) => ticket.id === command.ticket_id)
  )
    state.comments.push({
      id: String(command.local_id),
      ticket_id: String(command.ticket_id),
      markdown: String(command.markdown),
      created_at: stamp,
    });
  if (command.kind === "set_view") {
    result.view_states[String(command.context)] = command.view as View;
    state.views[String(command.context)] = command.view as View;
  }
  if (command.kind === "set_settings") {
    result.settings = command.settings as Snapshot["settings"];
    state.settings = result.settings;
  }
  if (
    command.kind === "set_board_options" &&
    command.board_id === state.boards[0]?.id
  )
    state.board_options[String(command.board_id)] =
      command.options as Workspace["board_options"][string];
  if (command.kind === "upsert_type") {
    const value = command.value as Workspace["types"][number];
    state.types = state.types
      .filter((type) => type.id !== value.id)
      .concat(value);
  }
  if (command.kind === "upsert_label") {
    const value = command.value as Workspace["labels"][number];
    state.labels = state.labels
      .filter((label) => label.id !== value.id)
      .concat(value);
  }
  if (
    command.kind === "create_column" &&
    command.board_id === state.boards[0]?.id
  )
    state.columns.push({
      id: String(command.local_id),
      board_id: String(command.board_id),
      name: String(command.name),
      position: Math.max(-1, ...state.columns.map((c) => c.position)) + 1,
      is_done: !!command.is_done,
      wip_limit: command.wip_limit as number | null,
      wip_strict: !!command.wip_strict,
    });
  if (command.kind === "update_column") {
    const column = state.columns.find((column) => column.id === command.id);
    if (column) {
      Object.assign(column, {
        name: command.name,
        is_done: command.is_done,
        wip_limit: command.wip_limit,
        wip_strict: command.wip_strict,
      });
      state.board_options[column.board_id].column_colors[column.id] = String(
        command.color,
      );
    }
  }
  if (command.kind === "delete_column")
    state.columns = state.columns.filter((column) => column.id !== command.id);
  if (command.kind === "reorder_columns")
    for (const column of state.columns) {
      const position = (command.ids as string[]).indexOf(column.id);
      if (position >= 0) column.position = position;
    }
  if (command.kind === "reorder_tickets")
    for (const ticket of state.tickets) {
      const position = (command.ids as string[]).indexOf(ticket.id);
      if (position >= 0) ticket.details.rank = position * 1024;
    }
  if (command.kind === "update_board") {
    const board = state.boards.find((board) => board.id === command.id);
    if (board) {
      board.name = String(command.name);
      board.revision++;
    }
  }
  if (command.kind === "batch")
    for (const action of command.commands as Command[]) {
      const nested = optimistic(result, { ...entry, action }, [
        result,
        ...cached.filter(
          (item) =>
            item.workspace.boards[0]?.id !== result.workspace.boards[0]?.id,
        ),
      ]);
      Object.assign(result, nested);
    }
  return result;
}

export function optimisticBootstrap(
  input: Bootstrap,
  entry: QueueEntry,
): Bootstrap {
  const result = structuredClone(input),
    command = entry.action;
  if (command.kind === "set_settings")
    result.settings = command.settings as Bootstrap["settings"];
  if (command.kind === "set_view")
    result.views[String(command.context)] = command.view as View;
  if (command.kind === "set_board_options")
    result.board_options[String(command.board_id)] =
      command.options as Bootstrap["board_options"][string];
  if (command.kind === "upsert_type") {
    const value = command.value as Bootstrap["types"][number];
    result.types = result.types
      .filter((type) => type.id !== value.id)
      .concat(value);
  }
  if (command.kind === "upsert_label") {
    const value = command.value as Bootstrap["labels"][number];
    result.labels = result.labels
      .filter((label) => label.id !== value.id)
      .concat(value);
  }
  if (command.kind === "create_board") {
    const board = {
      id: String(command.local_id),
      name: String(command.name),
      prefix: String(command.prefix),
      revision: 0,
      next_ticket_number: 1,
    };
    result.boards.push(board);
    result.board_options[board.id] = defaultOptions();
  }
  if (command.kind === "update_board") {
    const board = result.boards.find((board) => board.id === command.id);
    if (board) {
      board.name = String(command.name);
      board.revision++;
    }
  }
  return result;
}

const deadlineCache = new Map<string, number>();
export function deadlineTime(value: string | null, timezone: string) {
  if (!value) return Infinity;
  if (value.includes("T")) return Date.parse(value);
  const key = `${timezone}:${value}`;
  if (deadlineCache.has(key)) return deadlineCache.get(key)!;
  const format = new Intl.DateTimeFormat("en-CA", {
    timeZone: timezone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
  const localDate = (time: number) => {
    const parts = Object.fromEntries(
      format.formatToParts(time).map((part) => [part.type, part.value]),
    );
    return `${parts.year}-${parts.month}-${parts.day}`;
  };
  let low = Date.parse(`${value}T00:00:00Z`) - 36 * 3600000;
  let high = low + 4 * 86400000;
  while (high - low > 1) {
    const middle = Math.floor((high + low) / 2);
    if (localDate(middle) > value) high = middle;
    else low = middle;
  }
  deadlineCache.set(key, low);
  return low;
}
export function blocked(state: Workspace, id: string) {
  return state.dependencies.some(
    (edge) =>
      edge.ticket_id === id &&
      !state.tickets.find((ticket) => ticket.id === edge.prerequisite_id)?.done,
  );
}
export function score(state: Workspace, ticket: Ticket) {
  const options = state.board_options[ticket.board_id];
  const type = state.types.find((type) => type.id === ticket.details.type_id);
  const deadline = deadlineTime(
    ticket.details.deadline,
    state.settings.timezone,
  );
  const urgency = Number.isFinite(deadline)
    ? Math.max(
        0,
        Math.min(255, 30 - Math.trunc((deadline - Date.now()) / 86400000)),
      )
    : 0;
  return (
    ticket.importance * (options?.importance_weight ?? 1) +
    (type?.rank ?? 0) * (options?.type_weight ?? 1) +
    urgency * (options?.deadline_weight ?? 1)
  );
}
export function visibleTickets(state: Workspace, view: View) {
  const terms = view.query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const exact = state.tickets.find(
    (ticket) =>
      ticket.id === view.query.trim() ||
      ticket.public_id.toLowerCase() === view.query.trim().toLowerCase(),
  );
  const tickets = state.tickets.filter((ticket) => {
    const details = ticket.details;
    const active = !details.archived && !details.trashed;
    const status =
      view.status === "all" ||
      (view.status === "archive"
        ? details.archived && !details.trashed
        : view.status === "trash"
          ? details.trashed
          : view.status === "done"
            ? active && ticket.done
            : view.status === "open"
              ? active && !ticket.done
              : view.status === "blocked"
                ? active && blocked(state, ticket.id)
                : view.status === "backlog"
                  ? active && ticket.column_id === null
                  : view.status === "overdue"
                    ? active &&
                      !ticket.done &&
                      deadlineTime(details.deadline, state.settings.timezone) <
                        Date.now()
                    : active);
    if (
      !status ||
      (view.type_id && details.type_id !== view.type_id) ||
      !view.labels.every((label) => details.labels.includes(label))
    )
      return false;
    if (
      (view.importance_min != null &&
        ticket.importance < view.importance_min) ||
      (view.importance_max != null && ticket.importance > view.importance_max)
    )
      return false;
    if (
      !Object.entries(view.custom_fields ?? {}).every(([name, value]) =>
        details.custom_fields[name]
          ?.toLowerCase()
          .includes(value.toLowerCase()),
      )
    )
      return false;
    const deadline = deadlineTime(details.deadline, state.settings.timezone);
    if (view.deadline_from) {
      const previous = new Date(`${view.deadline_from}T12:00:00Z`);
      previous.setUTCDate(previous.getUTCDate() - 1);
      if (
        !Number.isFinite(deadline) ||
        deadline <
          deadlineTime(
            previous.toISOString().slice(0, 10),
            state.settings.timezone,
          ) +
            1
      )
        return false;
    }
    if (
      view.deadline_to &&
      (!Number.isFinite(deadline) ||
        deadline > deadlineTime(view.deadline_to, state.settings.timezone))
    )
      return false;
    if (exact) return exact.id === ticket.id;
    const labels = state.labels
      .filter((label) => details.labels.includes(label.id))
      .map((label) => label.name)
      .join(" ");
    const text =
      `${ticket.public_id} ${ticket.title} ${ticket.description} ${labels} ${JSON.stringify(details.custom_fields)}`.toLowerCase();
    return terms.every((term) => text.includes(term));
  });
  return tickets.sort((a, b) => {
    let result = 0;
    if (view.sort === "importance") result = a.importance - b.importance;
    else if (view.sort === "score") result = score(state, a) - score(state, b);
    else if (view.sort === "deadline") {
      const ad = deadlineTime(a.details.deadline, state.settings.timezone);
      const bd = deadlineTime(b.details.deadline, state.settings.timezone);
      result = ad === bd ? 0 : ad < bd ? -1 : 1;
    } else if (view.sort === "title" || view.sort === "public_id")
      result = a[view.sort]
        .toLowerCase()
        .localeCompare(b[view.sort].toLowerCase());
    else if (view.sort === "created" || view.sort === "updated")
      result = a.details[`${view.sort}_at`].localeCompare(
        b.details[`${view.sort}_at`],
      );
    else if (view.sort === "type")
      result = (a.details.type_id ?? "").localeCompare(b.details.type_id ?? "");
    else if (view.sort.startsWith("custom:"))
      result = (
        a.details.custom_fields[view.sort.slice(7)] ?? ""
      ).localeCompare(b.details.custom_fields[view.sort.slice(7)] ?? "");
    else result = a.details.rank - b.details.rank;
    return (
      (view.descending ? -result : result) ||
      a.public_id.localeCompare(b.public_id)
    );
  });
}

export function commandContext(state: Workspace, command: Command): unknown {
  switch (command.kind) {
    case "set_settings":
      return state.settings;
    case "set_board_options":
      return state.board_options[String(command.board_id)] ?? null;
    case "update_column":
    case "delete_column":
      return state.columns.find((column) => column.id === command.id) ?? null;
    case "reorder_columns":
      return state.columns
        .filter((column) => column.board_id === command.board_id)
        .sort((a, b) => a.position - b.position);
    case "reorder_tickets":
      return state.tickets
        .filter(
          (ticket) =>
            ticket.board_id === command.board_id &&
            ticket.column_id === command.column_id &&
            !ticket.details.parent_id &&
            !ticket.details.archived &&
            !ticket.details.trashed,
        )
        .sort((a, b) => a.details.rank - b.details.rank)
        .map((ticket) => [ticket.id, ticket.details.rank]);
    case "upsert_type":
      return (
        state.types.find(
          (type) => type.id === (command.value as { id: string }).id,
        ) ?? null
      );
    case "upsert_label":
      return (
        state.labels.find(
          (label) => label.id === (command.value as { id: string }).id,
        ) ?? null
      );
    case "upsert_recurrence":
      return (
        state.recurrences.find(
          (rule) => rule.id === (command.value as { id: string }).id,
        ) ?? null
      );
    case "delete_recurrence":
      return state.recurrences.find((rule) => rule.id === command.id) ?? null;
    case "set_view":
      return state.views[String(command.context)] ?? null;
    case "remove_dependency":
      return state.dependencies.some(
        (edge) =>
          edge.ticket_id === command.ticket_id &&
          edge.prerequisite_id === command.prerequisite_id,
      );
    case "remove_link":
      return state.links.some(
        (link) =>
          link.ticket_id === command.ticket_id &&
          link.target_id === command.target_id,
      );
    case "batch":
      return (command.commands as Command[]).map((command) =>
        commandContext(state, command),
      );
    default:
      return null;
  }
}
