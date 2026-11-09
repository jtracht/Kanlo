import { useEffect, useState } from "preact/hooks";
import type { QueueEntry, Snapshot, Receipt } from "./types";
import {
  api,
  connection,
  flush,
  resolve,
  loadBoard,
  HttpError,
} from "./client";
import * as storage from "./storage";
import { uuid } from "./identifiers";
import { Dialog, Message, useT, errorText, Busy } from "./ui";

export function QueueDialog({
  entries,
  close,
  preview,
  refreshed,
}: {
  entries: QueueEntry[];
  close: () => void;
  preview: (entry: QueueEntry) => void;
  refreshed: () => void;
}) {
  const t = useT();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function action(callback: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await callback();
      refreshed();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function discard(entry: QueueEntry) {
    if (
      !window.confirm(
        t(
          "Diese lokale Änderung endgültig verwerfen? Sie wird nicht zum Server übertragen.",
          "Permanently discard this local edit? It will not be sent to the server.",
        ),
      )
    )
      return;
    await storage.removeEntry(entry);
    if (connection.online) await loadBoard(entry.board_id);
  }
  return (
    <Dialog title={t("Lokale Änderungen", "Local changes")} close={close} wide>
      <div class="dialog-body">
        <p>
          {t(
            "Änderungen bleiben bis zur erfolgreichen Bestätigung gespeichert. Konflikte werden nicht automatisch überschrieben.",
            "Edits are retained until acknowledged. Conflicts are never overwritten automatically.",
          )}
        </p>
        {error && <Message error>{error}</Message>}
        {!entries.length && (
          <Message>
            {t("Alles synchronisiert.", "Everything is synchronized.")}
          </Message>
        )}
        {entries.map((entry, index) => (
          <section class="queue-entry" key={entry.op_id}>
            <div class="inline spread">
              <strong>
                {index + 1}. {actionName(entry.action.kind, t)}
              </strong>
              <span class="badge">
                {entry.state === "pending"
                  ? t("Wartet", "Pending")
                  : entry.state === "confirmation"
                    ? t("Bestätigung nötig", "Confirmation required")
                    : t("Prüfung nötig", "Review required")}
              </span>
            </div>
            <p class="muted">{new Date(entry.created_at).toLocaleString()}</p>
            {entry.state === "epoch_review" && (
              <Message>
                {t(
                  "Die Daten auf dem Server wurden wiederhergestellt oder ersetzt. Prüfe diese Änderung einzeln; gelöschte Tickets werden nicht automatisch neu angelegt.",
                  "Server data was restored or replaced. Review this edit individually; deleted tickets are never recreated automatically.",
                )}
              </Message>
            )}
            {entry.message && entry.message !== "conflict" && (
              <p>{entry.message}</p>
            )}
            {!!entry.conflicts?.length && (
              <div class="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>{t("Feld", "Field")}</th>
                      <th>{t("Basis", "Base")}</th>
                      <th>{t("Lokal", "Local")}</th>
                      <th>{t("Server", "Server")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {entry.conflicts.map((conflict) => (
                      <tr key={conflict.field}>
                        <th>{conflict.field}</th>
                        <td>
                          <pre>{display(conflict.base)}</pre>
                        </td>
                        <td>
                          <pre>{display(conflict.local)}</pre>
                        </td>
                        <td>
                          <pre>{display(conflict.server)}</pre>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            {entry.endpoint !== "/sync/patch" && entry.state !== "pending" && (
              <details>
                <summary>
                  {t("Geplante Änderung ansehen", "Inspect planned change")}
                </summary>
                <pre class="data-preview">
                  {JSON.stringify(entry.action, null, 2)}
                </pre>
              </details>
            )}
            <div class="actions wrap">
              {entry.state === "confirmation" && (
                <button
                  disabled={busy || !connection.online || index !== 0}
                  onClick={() => preview(entry)}
                >
                  {t("Vorschau prüfen", "Review preview")}
                </button>
              )}
              {entry.endpoint === "/sync/patch" &&
                ["conflict", "epoch_review"].includes(entry.state) && (
                  <>
                    <button
                      disabled={busy || !connection.online || index !== 0}
                      onClick={() => action(() => resolve(entry, "local"))}
                    >
                      {t(
                        "Lokale Werte erneut anfordern",
                        "Request local values again",
                      )}
                    </button>
                    <button
                      class="secondary"
                      disabled={busy || !connection.online || index !== 0}
                      onClick={() => action(() => resolve(entry, "server"))}
                    >
                      {t(
                        "Serverwerte für Konflikte behalten",
                        "Keep server values for conflicts",
                      )}
                    </button>
                  </>
                )}
              <button
                class="danger secondary"
                disabled={busy}
                onClick={() => action(() => discard(entry))}
              >
                {t("Diese Änderung verwerfen", "Discard this edit")}
              </button>
            </div>
          </section>
        ))}
      </div>
      <footer class="dialog-footer">
        <button class="secondary" onClick={close}>
          {t("Schließen", "Close")}
        </button>
        <button
          disabled={busy || connection.authRequired}
          onClick={() => action(flush)}
        >
          {busy ? (
            <Busy>{t("Synchronisieren", "Synchronizing")}</Busy>
          ) : (
            t("Jetzt synchronisieren", "Synchronize now")
          )}
        </button>
      </footer>
    </Dialog>
  );
}
function display(value: unknown) {
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}
export function actionName(
  kind: string,
  t: (de: string, en: string) => string,
) {
  const names: Record<string, [string, string]> = {
    create_ticket: ["Ticket anlegen", "Create ticket"],
    patch_ticket: ["Ticket bearbeiten", "Edit ticket"],
    move_ticket: ["Ticket verschieben", "Move ticket"],
    set_done: ["Status ändern", "Change status"],
    add_comment: ["Kommentar hinzufügen", "Add comment"],
    trash: ["In Papierkorb verschieben", "Move to trash"],
    restore: ["Wiederherstellen", "Restore"],
    purge: ["Endgültig löschen", "Permanently delete"],
    archive: ["Archivierung ändern", "Change archive state"],
    set_view: ["Ansicht speichern", "Save view"],
    set_settings: ["Einstellungen ändern", "Change settings"],
    set_board_options: ["Boardregeln ändern", "Change board rules"],
    create_board: ["Board anlegen", "Create board"],
    create_column: ["Spalte anlegen", "Create column"],
    update_column: ["Spalte bearbeiten", "Edit column"],
    delete_column: ["Spalte löschen", "Delete column"],
    delete_board: ["Board löschen", "Delete board"],
    batch: ["Sammelaktion", "Bulk action"],
    upsert_type: ["Tickettyp speichern", "Save ticket type"],
    upsert_label: ["Globales Label speichern", "Save global label"],
    clone_ticket: ["Ticket klonen", "Clone ticket"],
    add_dependency: ["Voraussetzung hinzufügen", "Add prerequisite"],
    remove_dependency: ["Voraussetzung entfernen", "Remove prerequisite"],
    add_link: ["Verknüpfung hinzufügen", "Add link"],
    upsert_recurrence: ["Wiederholung speichern", "Save recurrence"],
  };
  return names[kind] ? t(...names[kind]) : t("Änderung", "Change");
}

export function PreviewDialog({
  entry,
  close,
  refreshed,
}: {
  entry: QueueEntry;
  close: () => void;
  refreshed: () => void;
}) {
  const t = useT();
  const [preview, setPreview] = useState<{
    token: string;
    result: { affected: string[]; boards: string[]; warnings: string[] };
    removed_dependencies: unknown[];
    tickets: { id: string; public_id: string; title: string }[];
  }>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function load() {
    setBusy(true);
    setError("");
    try {
      try {
        const receipt = await api<Receipt>(
          `/sync/receipts/${entry.op_id}?client_id=${encodeURIComponent(String(entry.body.client_id))}`,
        );
        await storage.acknowledge(entry, {}, receipt);
        await loadBoard(entry.board_id);
        refreshed();
        close();
        return;
      } catch (error) {
        if (!(error instanceof HttpError && error.status === 404)) throw error;
      }
      setPreview(
        await api("/commands/preview", {
          command: entry.body.command,
          base_context: entry.body.base_context,
        }),
      );
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    if (connection.online) void load();
  }, []);
  async function confirm() {
    if (!preview) return;
    setBusy(true);
    setError("");
    try {
      const receipt = await api<Receipt>("/commands/confirm", {
        token: preview.token,
        op_id: entry.op_id,
        client_id: String(entry.body.client_id),
      });
      await storage.acknowledge(entry, {}, receipt);
      window.dispatchEvent(
        new CustomEvent("kanlo:receipt", { detail: receipt }),
      );
      refreshed();
      close();
      void flush();
      await loadBoard(entry.board_id);
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog title={t("Änderung bestätigen", "Confirm change")} close={close}>
      <div class="dialog-body">
        <h3>{actionName(entry.action.kind, t)}</h3>
        {error && <Message error>{error}</Message>}
        {!connection.online && (
          <Message>
            {t(
              "Die Anfrage ist lokal gespeichert. Verbinde dich wieder und prüfe dann die endgültigen Auswirkungen.",
              "The request is stored locally. Reconnect and review its final effects.",
            )}
          </Message>
        )}
        {preview && (
          <>
            <p>
              {preview.result.affected.length}{" "}
              {t("betroffene Tickets", "affected tickets")} ·{" "}
              {preview.result.boards.length} {t("Boards", "boards")}
            </p>
            <p>
              {preview.removed_dependencies.length}{" "}
              {t(
                "Abhängigkeiten werden entfernt.",
                "dependencies will be removed.",
              )}
            </p>
            {preview.result.warnings.map((warning) => (
              <Message key={warning}>{warning}</Message>
            ))}
            <details>
              <summary>
                {t("Betroffene Kennungen", "Affected identifiers")}
              </summary>
              <ul>
                {preview.result.affected.map((id) => (
                  <li key={id}>{id}</li>
                ))}
              </ul>
            </details>
            <Message>
              {t(
                "Erst die folgende Bestätigung führt die Aktion aus. Bei zwischenzeitlichen Änderungen ist eine neue Vorschau nötig.",
                "Only the confirmation below executes the action. Changes in the meantime require a new preview.",
              )}
            </Message>
          </>
        )}
      </div>
      <footer class="dialog-footer">
        <button
          class="secondary"
          onClick={async () => {
            await storage.removeEntry(entry);
            refreshed();
            close();
            void flush();
          }}
        >
          {t("Anfrage abbrechen", "Cancel request")}
        </button>
        <button
          class="secondary"
          disabled={busy || !connection.online}
          onClick={load}
        >
          {t("Vorschau aktualisieren", "Refresh preview")}
        </button>
        <button
          disabled={!preview || busy || !connection.online}
          class="danger"
          onClick={confirm}
        >
          {busy
            ? t("Bitte warten …", "Please wait…")
            : t("Bestätigen und ausführen", "Confirm and execute")}
        </button>
      </footer>
    </Dialog>
  );
}
