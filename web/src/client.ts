import { uuid } from "./identifiers";
import type {
  Bootstrap,
  Command,
  QueueEntry,
  Receipt,
  Snapshot,
  Ticket,
} from "./types";
import { fields } from "./types";
import * as storage from "./storage";
import { commandContext } from "./workspace";

export class HttpError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public data?: unknown,
  ) {
    super(message);
  }
}
let csrf: string | undefined;
export const connection = {
  online: navigator.onLine,
  authRequired: false,
  syncing: false,
  error: "",
};
export function stateChanged() {
  storage.changed();
}
export async function api<T = unknown>(
  path: string,
  body?: unknown,
  method = body === undefined ? "GET" : "POST",
): Promise<T> {
  const headers: Record<string, string> = {};
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (csrf) headers["X-Kanlo-CSRF"] = csrf;
  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: "same-origin",
      signal: AbortSignal.timeout(15000),
      cache: "no-store",
    });
  } catch (error) {
    connection.online = false;
    throw error;
  }
  if (!response.ok) {
    const value = await response.json().catch(() => null);
    if (
      response.status === 401 ||
      (response.status === 403 &&
        ["AUTH_EXPIRED", "CSRF"].includes(value?.error?.code))
    ) {
      connection.authRequired = true;
      storage.changed();
    }
    if (value?.error?.code === "OFFLINE") connection.online = false;
    throw new HttpError(
      response.status,
      value?.error?.code ?? "HTTP",
      value?.error?.message ?? `HTTP ${response.status}`,
      value,
    );
  }
  connection.online = true;
  return response.json() as Promise<T>;
}
export async function authenticate(passcode?: string) {
  const session = await api<{ csrf?: string; authenticated: boolean }>(
    passcode === undefined ? "/session" : "/login",
    passcode === undefined ? undefined : { passcode },
  );
  csrf = session.csrf;
  connection.authRequired = false;
  storage.changed();
}
export async function loadBootstrap(): Promise<Bootstrap> {
  try {
    await authenticate();
    const value = await api<Bootstrap>("/bootstrap");
    await storage.saveBootstrap(value);
    return (await storage.bootstrapCache())!;
  } catch (error) {
    const cached = await storage.bootstrapCache();
    if (cached) return cached;
    throw error;
  }
}
export async function loadBoard(id: string): Promise<Snapshot> {
  if (id.startsWith("tmp:")) {
    const cached = await storage.boardCache(id);
    if (cached) return cached;
  }
  try {
    const snapshot = await api<Snapshot>(
      `/boards/${encodeURIComponent(id)}/snapshot`,
    );
    // A lost creation response may already be represented in this snapshot.
    // Resolve its durable receipt before projecting a second temporary card.
    for (const entry of await storage.queue())
      if (entry.endpoint === "/sync/create" && entry.board_id === id) {
        try {
          const receipt = await api<Receipt>(
            `/sync/receipts/${entry.op_id}?client_id=${encodeURIComponent(String(entry.body.client_id))}`,
          );
          await storage.acknowledge(entry, receipt.temp_ids, receipt);
        } catch (error) {
          if (!(error instanceof HttpError && error.status === 404))
            throw error;
        }
      }
    await storage.storeSnapshot(id, snapshot);
    return (await storage.boardCache(id))!;
  } catch (error) {
    const cached = await storage.boardCache(id);
    if (cached) return cached;
    throw error;
  }
}

async function base() {
  const bootstrap = await storage.bootstrapCache();
  if (!bootstrap) throw new Error("Initialisierung fehlt.");
  return {
    op_id: uuid(),
    client_id: await storage.clientId(),
    server_instance_id: bootstrap.instance_id,
    base_epoch: bootstrap.epoch,
  };
}
export async function queueCommand(
  command: Command,
  board_id: string,
  source: Snapshot,
  confirmation = false,
) {
  const metadata = await base();
  metadata.base_epoch = source.epoch;
  const action = { ...command };
  const bodyCommand = { ...command };
  delete bodyCommand.local_id;
  if (
    ["create_board", "create_column"].includes(action.kind) &&
    !action.local_id
  )
    action.local_id = `tmp:${uuid()}`;
  const body = {
    ...metadata,
    command: bodyCommand,
    base_context: commandContext(source.workspace, command),
  };
  const entry: QueueEntry = {
    ...metadata,
    endpoint: "/commands",
    body,
    board_id,
    action,
    created_at: new Date().toISOString(),
    state: confirmation ? "confirmation" : "pending",
  };
  await storage.enqueue(entry);
  const stored = await (
    await storage.db
  ).getFromIndex("queue", "op_id", metadata.op_id);
  void flush();
  return stored ?? entry;
}
export async function createTicket(command: Command, board: string) {
  const metadata = await base();
  metadata.base_epoch =
    (await storage.boardCache(board))?.epoch ?? metadata.base_epoch;
  const temporary_id = `tmp:${uuid()}`;
  const body = {
    ...metadata,
    temporary_id,
    board_id: board,
    title: command.title,
    description: command.description ?? "",
    importance: command.importance ?? 0,
    column_id: command.column_id ?? null,
    parent_id: command.parent_id ?? null,
    type_id: command.type_id ?? null,
  };
  await storage.enqueue({
    ...metadata,
    endpoint: "/sync/create",
    body,
    board_id: board,
    action: {
      ...command,
      kind: "create_ticket",
      board_id: board,
      local_id: temporary_id,
    },
    state: "pending",
    created_at: new Date().toISOString(),
  });
  void flush();
  return temporary_id;
}
export async function patchTicket(
  ticket: Ticket,
  changed_fields: Record<string, unknown>,
  extraBase?: Record<string, unknown>,
  epoch?: string,
) {
  const metadata = await base();
  metadata.base_epoch =
    epoch ??
    (await storage.boardCache(ticket.board_id))?.epoch ??
    metadata.base_epoch;
  const current = {
    ...fields(ticket),
    board_id: ticket.board_id,
    column_id: ticket.column_id,
    done: ticket.done,
    archived: ticket.details.archived,
    ...extraBase,
  };
  const base_fields = Object.fromEntries(
    Object.keys(changed_fields).map((name) => [
      name,
      current[name as keyof typeof current],
    ]),
  );
  const body = {
    ...metadata,
    entity_id: ticket.id,
    base_revision: ticket.revision,
    base_fields,
    changed_fields,
    created_at: new Date().toISOString(),
  };
  let action: Command = {
    kind: "patch_ticket",
    id: ticket.id,
    fields: changed_fields,
  };
  if ("column_id" in changed_fields || "board_id" in changed_fields)
    action = { kind: "move_ticket", id: ticket.id, ...changed_fields };
  else if ("done" in changed_fields)
    action = { kind: "set_done", id: ticket.id, done: changed_fields.done };
  else if ("archived" in changed_fields)
    action = {
      kind: "archive",
      ids: [ticket.id],
      archived: changed_fields.archived,
    };
  await storage.enqueue({
    ...metadata,
    endpoint: "/sync/patch",
    body,
    board_id: ticket.board_id,
    action,
    state: "pending",
    created_at: body.created_at,
  });
  void flush();
}
export async function addComment(ticket: Ticket, markdown: string) {
  const metadata = await base();
  metadata.base_epoch =
    (await storage.boardCache(ticket.board_id))?.epoch ?? metadata.base_epoch;
  const local_id = `tmp:${uuid()}`;
  await storage.enqueue({
    ...metadata,
    endpoint: "/sync/comment",
    body: { ...metadata, entity_id: ticket.id, markdown },
    board_id: ticket.board_id,
    action: { kind: "add_comment", ticket_id: ticket.id, markdown, local_id },
    state: "pending",
    created_at: new Date().toISOString(),
  });
  void flush();
}

const owner = uuid();
let activeFlush: Promise<void> | undefined;
async function synchronize(useLease = true) {
  if (!(await storage.queue()).length) return;
  if (
    !navigator.onLine ||
    connection.authRequired ||
    (useLease && !(await storage.claimLease(owner)))
  )
    return;
  connection.syncing = true;
  storage.changed();
  try {
    await authenticate();
    const handshake = await api<{
      instance_id: string;
      epoch: string;
      revision: number;
    }>("/sync/handshake");
    connection.error = "";
    while (true) {
      const entry = (await storage.queue())[0];
      if (!entry) break;
      if (entry.state !== "pending") break;
      let acknowledged = false;
      if (useLease && !(await storage.claimLease(owner))) break;
      if (
        entry.body.server_instance_id !== handshake.instance_id ||
        entry.body.base_epoch !== handshake.epoch
      ) {
        await storage.updateEntry({
          ...entry,
          state: "epoch_review",
          message:
            "Serverinstanz oder Datenepoche geändert. Änderung einzeln prüfen.",
        });
        break;
      }
      try {
        const response = await api<
          | Receipt
          | {
              status: string;
              receipt?: Receipt;
              conflicts: QueueEntry["conflicts"];
              epoch_changed: boolean;
            }
        >(entry.endpoint, entry.body);
        if ("status" in response && response.status !== "applied") {
          await storage.updateEntry({
            ...entry,
            state:
              response.status === "epoch_review" ? "epoch_review" : "conflict",
            conflicts: response.conflicts,
            message: response.status,
          });
          break;
        }
        const receipt =
          "receipt" in response ? response.receipt! : (response as Receipt);
        const mapping = { ...receipt.temp_ids };
        if (entry.action.local_id && receipt.result.entity_id)
          mapping[String(entry.action.local_id)] = receipt.result.entity_id;
        if (!(await storage.acknowledge(entry, mapping, receipt))) break;
        acknowledged = true;
        window.dispatchEvent(
          new CustomEvent("kanlo:receipt", { detail: receipt }),
        );
        const bootstrap = await api<Bootstrap>("/bootstrap");
        await storage.saveBootstrap(bootstrap);
        const database = await storage.db;
        const cached = await database.getAllKeys("boards");
        for (const board of receipt.result.boards)
          if (cached.includes(board)) {
            try {
              const snapshot = await api<Snapshot>(
                `/boards/${encodeURIComponent(board)}/snapshot`,
              );
              await storage.storeSnapshot(board, snapshot);
            } catch (error) {
              if (error instanceof HttpError && error.status === 404)
                await storage.removeBoard(board);
              else throw error;
            }
          }
      } catch (error) {
        if (
          !acknowledged &&
          error instanceof HttpError &&
          error.status !== 401 &&
          error.status !== 403 &&
          error.status !== 429 &&
          error.status < 500
        ) {
          await storage.updateEntry({
            ...entry,
            state: /epoch|instance/i.test(error.message)
              ? "epoch_review"
              : "conflict",
            message: error.message,
          });
        } else {
          connection.error =
            error instanceof Error ? error.message : String(error);
        }
        break;
      }
    }
  } catch (error) {
    connection.error = error instanceof Error ? error.message : String(error);
  } finally {
    connection.syncing = false;
    if (useLease) await storage.releaseLease(owner);
    storage.changed();
  }
}
export function flush() {
  if (activeFlush) return activeFlush;
  activeFlush = (async () => {
    if (navigator.locks)
      await navigator.locks.request("kanlo-sync", () => synchronize(false));
    else await synchronize();
  })().finally(() => {
    activeFlush = undefined;
  });
  return activeFlush;
}
export function startSynchronization(onChange: () => void) {
  let events: EventSource | undefined;
  let checkingSession = false;
  const connect = () => {
    if (events || connection.authRequired) return;
    events = new EventSource("/api/v1/events", { withCredentials: true });
    let timer: number | undefined;
    events.addEventListener("change", () => {
      clearTimeout(timer);
      timer = window.setTimeout(onChange, 200);
    });
    events.addEventListener("auth_expired", () => {
      connection.authRequired = true;
      events?.close();
      events = undefined;
      storage.changed();
    });
    events.addEventListener("error", () => {
      // HTTP 401 on the initial SSE request cannot emit auth_expired. Check the
      // real session without turning a network outage into an authentication error.
      if (!navigator.onLine || checkingSession || connection.authRequired) return;
      checkingSession = true;
      void api("/session")
        .catch(() => {
          if (connection.authRequired) {
            events?.close();
            events = undefined;
          }
        })
        .finally(() => {
          checkingSession = false;
        });
    });
  };
  const online = () => {
    connection.online = navigator.onLine;
    storage.changed();
    if (navigator.onLine) {
      connect();
      void flush();
    }
  };
  window.addEventListener("online", online);
  window.addEventListener("offline", online);
  const interval = window.setInterval(() => {
    if (navigator.onLine && !connection.authRequired) void flush();
  }, 15000);
  if (navigator.onLine) {
    connect();
  }
  void flush();
  return () => {
    window.removeEventListener("online", online);
    window.removeEventListener("offline", online);
    clearInterval(interval);
    events?.close();
  };
}

export async function resolve(entry: QueueEntry, choice: "local" | "server") {
  if (entry.endpoint !== "/sync/patch")
    throw new Error("Strukturänderung nach Prüfung erneut ausführen.");
  const id = String(entry.body.entity_id);
  const ticket = await api<Ticket>(`/tickets/${encodeURIComponent(id)}`);
  const changed = structuredClone(entry.body.changed_fields) as Record<
    string,
    unknown
  >;
  const current = {
    ...fields(ticket),
    board_id: ticket.board_id,
    column_id: ticket.column_id,
    done: ticket.done,
    archived: ticket.details.archived,
  };
  if (choice === "server")
    for (const field of entry.state === "epoch_review"
      ? Object.keys(changed)
      : (entry.conflicts ?? []).map((conflict) => conflict.field))
      changed[field] = current[field as keyof typeof current];
  await loadBootstrap();
  const metadata = await base();
  const replacement: QueueEntry = {
    ...entry,
    ...metadata,
    seq: undefined,
    op_id: metadata.op_id,
    state: "pending",
    conflicts: undefined,
    message: undefined,
    action: {
      ...entry.action,
      ...changed,
      ...(entry.action.kind === "patch_ticket" ? { fields: changed } : {}),
    },
    body: {
      ...entry.body,
      ...metadata,
      base_revision: ticket.revision,
      base_fields: Object.fromEntries(
        Object.keys(changed).map((key) => [
          key,
          current[key as keyof typeof current],
        ]),
      ),
      changed_fields: changed,
    },
  };
  // Replace atomically: a crash cannot lose the old operation before the new one is durable.
  replacement.seq = entry.seq;
  await storage.updateEntry(replacement);
  await flush();
}
