import { uuid } from "./identifiers";
import { openDB, type DBSchema, type IDBPTransaction } from "idb";
import type {
  Bootstrap,
  EntityChange,
  QueueEntry,
  Receipt,
  Snapshot,
  Workspace,
} from "./types";
import { optimistic, optimisticBootstrap } from "./workspace";

interface Cache extends DBSchema {
  meta: { key: string; value: unknown };
  bases: { key: string; value: Snapshot };
  boards: { key: string; value: Snapshot };
  queue: { key: number; value: QueueEntry; indexes: { op_id: string } };
}
const stores = ["meta", "bases", "boards", "queue"] as const;
type Transaction = IDBPTransaction<Cache, typeof stores, "readwrite">;
export const db = openDB<Cache>("kanlo-client-v1", 2, {
  upgrade(db, previous, _next, tx) {
    if (previous < 1) {
      db.createObjectStore("meta");
      db.createObjectStore("boards");
      db.createObjectStore("queue", {
        keyPath: "seq",
        autoIncrement: true,
      }).createIndex("op_id", "op_id", { unique: true });
    }
    if (previous < 2) {
      db.createObjectStore("bases");
      if (previous === 1)
        void tx
          .objectStore("boards")
          .getAll()
          .then(async (snapshots) => {
            for (const snapshot of snapshots) {
              const id = snapshot.workspace.boards[0]?.id;
              if (id) await tx.objectStore("bases").put(snapshot, id);
            }
          });
    }
  },
});
export const channel =
  typeof BroadcastChannel !== "undefined"
    ? new BroadcastChannel("kanlo-client")
    : null;
const listeners = new Set<(mapping?: Record<string, string>) => void>();
export function changed(mapping?: Record<string, string>) {
  for (const listener of listeners) listener(mapping);
  channel?.postMessage({ kind: "changed", mapping });
}
channel?.addEventListener("message", (event) => {
  if (event.data?.kind === "changed")
    for (const listener of listeners) listener(event.data.mapping);
});
export function subscribe(
  listener: (mapping?: Record<string, string>) => void,
) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export async function clientId() {
  const transaction = (await db).transaction("meta", "readwrite");
  let id = await transaction.store.get("client_id");
  if (typeof id !== "string") {
    id = uuid();
    await transaction.store.put(id, "client_id");
  }
  await transaction.done;
  return id as string;
}
export async function bootstrapCache() {
  return (await db).get("meta", "bootstrap") as Promise<Bootstrap | undefined>;
}
export async function boardCache(id: string) {
  return (await db).get("boards", id);
}
export async function queue() {
  return (await db).getAll("queue");
}

async function rebuild(transaction: Transaction) {
  const source = (await transaction
    .objectStore("meta")
    .get("server_bootstrap")) as Bootstrap | undefined;
  if (!source) return;
  const entries = await transaction.objectStore("queue").getAll();
  let bootstrap = structuredClone(source);
  let snapshots = await transaction.objectStore("bases").getAll();
  for (const entry of entries) {
    if (
      entry.state === "epoch_review" ||
      entry.body.base_epoch !== source.epoch ||
      entry.body.server_instance_id !== source.instance_id
    )
      continue;
    bootstrap = optimisticBootstrap(bootstrap, entry);
    if (entry.action.kind === "create_board") {
      const board = bootstrap.boards.find(
        (board) => board.id === entry.action.local_id,
      );
      if (
        board &&
        !snapshots.some(
          (snapshot) => snapshot.workspace.boards[0]?.id === board.id,
        )
      )
        snapshots.push({
          workspace: {
            boards: [board],
            columns: [],
            tickets: [],
            types: bootstrap.types,
            labels: bootstrap.labels,
            comments: [],
            dependencies: [],
            links: [],
            board_options: { [board.id]: bootstrap.board_options[board.id] },
            views: bootstrap.views,
            settings: bootstrap.settings,
            recurrences: [],
          },
          external_references: [],
          revision: source.revision,
          settings: bootstrap.settings,
          view_states: bootstrap.views,
          epoch: source.epoch,
        });
    }
    const previous = snapshots;
    snapshots = previous.map((snapshot) =>
      optimistic(snapshot, entry, previous),
    );
  }
  await transaction.objectStore("meta").put(bootstrap, "bootstrap");
  await transaction.objectStore("boards").clear();
  for (const snapshot of snapshots) {
    const id = snapshot.workspace.boards[0]?.id;
    if (id) await transaction.objectStore("boards").put(snapshot, id);
  }
}
export async function saveBootstrap(value: Bootstrap) {
  const transaction = (await db).transaction(stores, "readwrite");
  const previous = (await transaction
    .objectStore("meta")
    .get("server_bootstrap")) as Bootstrap | undefined;
  if (
    previous &&
    previous.instance_id === value.instance_id &&
    previous.revision > value.revision
  ) {
    await transaction.done;
    return;
  }
  await transaction.objectStore("meta").put(value, "server_bootstrap");
  await rebuild(transaction);
  await transaction.done;
}
export async function storeSnapshot(id: string, snapshot: Snapshot) {
  const transaction = (await db).transaction(stores, "readwrite");
  const previous = await transaction.objectStore("bases").get(id);
  if (previous && previous.revision > snapshot.revision) {
    await transaction.done;
    return;
  }
  await transaction.objectStore("bases").put(snapshot, id);
  await rebuild(transaction);
  await transaction.done;
}
export async function removeBoard(id: string) {
  const transaction = (await db).transaction(stores, "readwrite");
  await transaction.objectStore("bases").delete(id);
  await rebuild(transaction);
  await transaction.done;
  changed();
}
export async function enqueue(entry: QueueEntry) {
  const transaction = (await db).transaction(stores, "readwrite");
  try {
    await transaction.objectStore("queue").add(entry);
    await rebuild(transaction);
    await transaction.done;
    changed();
  } catch (error) {
    try {
      transaction.abort();
    } catch {
      /* already aborted */
    }
    throw error;
  }
}
export async function updateEntry(entry: QueueEntry) {
  const transaction = (await db).transaction(stores, "readwrite");
  await transaction.objectStore("queue").put(entry);
  await rebuild(transaction);
  await transaction.done;
  changed();
}
export async function removeEntry(entry: QueueEntry) {
  const transaction = (await db).transaction(stores, "readwrite");
  if (entry.seq !== undefined)
    await transaction.objectStore("queue").delete(entry.seq);
  await rebuild(transaction);
  await transaction.done;
  changed();
}

const referenceKeys = new Set([
  "id",
  "entity_id",
  "board_id",
  "column_id",
  "parent_id",
  "type_id",
  "template_id",
  "ticket_id",
  "prerequisite_id",
  "target_id",
  "local_id",
  "last_created",
  "last_active_column",
]);
export function remap(
  value: unknown,
  mapping: Record<string, string>,
  key = "",
): unknown {
  if (typeof value === "string") {
    if (key === "context") {
      for (const [from, to] of Object.entries(mapping))
        if (value.endsWith(`:${from}`))
          return `${value.slice(0, -from.length)}${to}`;
    }
    return referenceKeys.has(key) || key === "base_context"
      ? mapping[value] || value
      : value;
  }
  if (Array.isArray(value))
    return value.map((item) =>
      typeof item === "string" &&
      ["ids", "labels", "enabled_types"].includes(key)
        ? mapping[item] || item
        : remap(item, mapping, key),
    );
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value).map(([childKey, child]) => {
        let nextKey = [
          "board_options",
          "column_colors",
          "convert_types",
        ].includes(key)
          ? mapping[childKey] || childKey
          : childKey;
        if (key === "views" || key === "view_states")
          nextKey = remap(childKey, mapping, "context") as string;
        return [
          nextKey,
          childKey === "custom_fields"
            ? child
            : remap(child, mapping, childKey),
        ];
      }),
    );
  return value;
}

function applyChange(snapshot: Snapshot, change: EntityChange) {
  const state = snapshot.workspace;
  const board = state.boards[0]?.id;
  const arrays: Record<string, keyof Workspace> = {
    board: "boards",
    column: "columns",
    ticket: "tickets",
    type: "types",
    label: "labels",
    comment: "comments",
    recurrence: "recurrences",
  };
  const field = arrays[change.kind];
  if (field) {
    const current = state[field] as unknown as { id: string }[];
    const updated = current.filter((value) => value.id !== change.id);
    if (
      change.body &&
      (change.kind === "board"
        ? change.id === board
        : ["type", "label"].includes(change.kind) || change.board_id === board)
    )
      updated.push(change.body as { id: string });
    (state as unknown as Record<string, unknown>)[field] = updated;
  } else if (change.kind === "board_options") {
    if (change.body)
      state.board_options[change.id] =
        change.body as Workspace["board_options"][string];
    else delete state.board_options[change.id];
  } else if (change.kind === "settings" && change.body) {
    state.settings = change.body as Workspace["settings"];
    snapshot.settings = state.settings;
  } else if (change.kind === "view") {
    if (change.body)
      state.views[change.id] = change.body as Workspace["views"][string];
    else delete state.views[change.id];
    snapshot.view_states = state.views;
  } else if (change.kind === "dependency" || change.kind === "link") {
    const key = change.kind === "dependency" ? "dependencies" : "links";
    const values = state[key] as {
      ticket_id: string;
      prerequisite_id?: string;
      target_id?: string;
    }[];
    const updated = values.filter(
      (value) =>
        `${value.ticket_id}:${value.prerequisite_id ?? value.target_id}` !==
        change.id,
    );
    if (change.body && change.board_id === board)
      updated.push(change.body as (typeof updated)[number]);
    (state as unknown as Record<string, unknown>)[key] = updated;
  }
}
function applyMetadata(bootstrap: Bootstrap, change: EntityChange) {
  if (change.kind === "settings" && change.body)
    bootstrap.settings = change.body as Bootstrap["settings"];
  if (change.kind === "view") {
    if (change.body)
      bootstrap.views[change.id] = change.body as Bootstrap["views"][string];
    else delete bootstrap.views[change.id];
  }
  if (change.kind === "board_options") {
    if (change.body)
      bootstrap.board_options[change.id] =
        change.body as Bootstrap["board_options"][string];
    else delete bootstrap.board_options[change.id];
  }
  if (change.kind === "board")
    bootstrap.boards = bootstrap.boards
      .filter((board) => board.id !== change.id)
      .concat(change.body ? [change.body as Bootstrap["boards"][number]] : []);
  if (change.kind === "type")
    bootstrap.types = bootstrap.types
      .filter((type) => type.id !== change.id)
      .concat(change.body ? [change.body as Bootstrap["types"][number]] : []);
  if (change.kind === "label")
    bootstrap.labels = bootstrap.labels
      .filter((label) => label.id !== change.id)
      .concat(change.body ? [change.body as Bootstrap["labels"][number]] : []);
}
export async function acknowledge(
  entry: QueueEntry,
  mapping: Record<string, string>,
  receipt: Receipt,
) {
  const transaction = (await db).transaction(stores, "readwrite");
  if (entry.seq !== undefined)
    await transaction.objectStore("queue").delete(entry.seq);
  if (Object.keys(mapping).length) {
    for (const queued of await transaction.objectStore("queue").getAll())
      await transaction
        .objectStore("queue")
        .put(remap(queued, mapping) as QueueEntry);
    for (const snapshot of await transaction.objectStore("bases").getAll()) {
      const id = snapshot.workspace.boards[0]?.id;
      if (!id) continue;
      const updated = remap(snapshot, mapping) as Snapshot;
      const next = updated.workspace.boards[0]?.id ?? id;
      if (next !== id) await transaction.objectStore("bases").delete(id);
      await transaction.objectStore("bases").put(updated, next);
    }
    const metadata = await transaction
      .objectStore("meta")
      .get("server_bootstrap");
    if (metadata)
      await transaction
        .objectStore("meta")
        .put(remap(metadata, mapping), "server_bootstrap");
  }
  const metadata = (await transaction
    .objectStore("meta")
    .get("server_bootstrap")) as Bootstrap | undefined;
  if (metadata) {
    for (const change of receipt.changes ?? []) applyMetadata(metadata, change);
    metadata.revision = receipt.revision;
    if (receipt.epoch) metadata.epoch = receipt.epoch;
    await transaction.objectStore("meta").put(metadata, "server_bootstrap");
    const snapshots = await transaction.objectStore("bases").getAll();
    if (
      entry.action.kind === "create_board" &&
      receipt.result.entity_id &&
      !snapshots.some(
        (snapshot) =>
          snapshot.workspace.boards[0]?.id === receipt.result.entity_id,
      )
    ) {
      const board = metadata.boards.find(
        (board) => board.id === receipt.result.entity_id,
      );
      if (board)
        snapshots.push({
          workspace: {
            boards: [board],
            columns: [],
            tickets: [],
            types: metadata.types,
            labels: metadata.labels,
            comments: [],
            dependencies: [],
            links: [],
            board_options: metadata.board_options,
            views: metadata.views,
            settings: metadata.settings,
            recurrences: [],
          },
          external_references: [],
          revision: receipt.revision,
          epoch: metadata.epoch,
          settings: metadata.settings,
          view_states: metadata.views,
        });
    }
    for (const snapshot of snapshots) {
      const id = snapshot.workspace.boards[0]?.id;
      if (!id) continue;
      for (const change of receipt.changes ?? []) applyChange(snapshot, change);
      snapshot.revision = receipt.revision;
      if (receipt.epoch) snapshot.epoch = receipt.epoch;
      if (snapshot.workspace.boards.length)
        await transaction.objectStore("bases").put(snapshot, id);
      else await transaction.objectStore("bases").delete(id);
    }
  }
  await rebuild(transaction);
  await transaction.done;
  changed(mapping);
}
export async function claimLease(owner: string) {
  const transaction = (await db).transaction("meta", "readwrite");
  const current = (await transaction.store.get("sync_lease")) as
    { owner: string; until: number } | undefined;
  if (current && current.owner !== owner && current.until > Date.now()) {
    await transaction.done;
    return false;
  }
  await transaction.store.put(
    { owner, until: Date.now() + 30000 },
    "sync_lease",
  );
  await transaction.done;
  return true;
}
export async function releaseLease(owner: string) {
  const transaction = (await db).transaction("meta", "readwrite");
  const current = (await transaction.store.get("sync_lease")) as
    { owner: string } | undefined;
  if (current?.owner === owner) await transaction.store.delete("sync_lease");
  await transaction.done;
}
export async function clearCache(confirmedPendingCount: number) {
  const transaction = (await db).transaction(stores, "readwrite");
  if (
    (await transaction.objectStore("queue").count()) !== confirmedPendingCount
  ) {
    transaction.abort();
    throw new Error("Warteschlange geändert / Queue changed; review again.");
  }
  await Promise.all(
    stores.map((name) => transaction.objectStore(name).clear()),
  );
  await transaction.done;
  changed();
}
