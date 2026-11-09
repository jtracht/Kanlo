import { useEffect, useState } from "preact/hooks";
import { api, loadBoard, connection } from "./client";
import { Dialog, Message, useT, errorText } from "./ui";
import { actionName } from "./QueueDialog";
import { clientId, acknowledge } from "./storage";
import type { Receipt } from "./types";
import { uuid } from "./identifiers";

interface Entry {
  id: number;
  description: string;
  created_at: string;
}
export function HistoryDialog({
  board,
  close,
  refresh,
}: {
  board: string;
  close: () => void;
  refresh: () => void;
}) {
  const t = useT();
  const [entries, setEntries] = useState<Entry[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState<{
    token: string;
    summary: { undo_groups: number; affected_tickets: string[] };
  }>();
  const [operation, setOperation] = useState(uuid());
  const [revision, setRevision] = useState(0);
  async function history() {
    const { revision } = await api<{ revision: number }>("/sync/handshake");
    const entries = await api<Entry[]>(`/undo/${board}`);
    setEntries(entries);
    setRevision(revision);
  }
  useEffect(() => {
    history().catch((error) => setError(errorText(error)));
  }, [board]);
  async function prepare(through_id: number) {
    setBusy(true);
    setError("");
    try {
      setPreview(await api(`/undo/${board}`, { revision, through_id }));
      setOperation(uuid());
    } catch (error) {
      setError(errorText(error));
      await history();
    } finally {
      setBusy(false);
    }
  }
  async function execute() {
    if (!preview) return;
    setBusy(true);
    setError("");
    try {
      const receipt = await api<Receipt>("/recovery/confirm", {
        token: preview.token,
        op_id: operation,
        client_id: await clientId(),
      });
      await acknowledge(
        {
          op_id: receipt.op_id,
          endpoint: "/recovery/confirm",
          body: {},
          action: { kind: "undo" },
          board_id: board,
          created_at: new Date().toISOString(),
          state: "pending",
        },
        {},
        receipt,
      );
      setPreview(undefined);
      await loadBoard(board);
      await history();
      refresh();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  function description(value: string) {
    try {
      const command = JSON.parse(value);
      return actionName(command.kind, t);
    } catch {
      return value;
    }
  }
  return (
    <Dialog title={t("Änderungsverlauf", "Change history")} close={close}>
      <div class="dialog-body">
        {error && <Message error>{error}</Message>}
        <p>
          {t(
            "Zusammengehörige Änderungen werden gemeinsam zurückgenommen. Spätere widersprüchliche Änderungen verhindern eine unsichere Rücknahme.",
            "Related changes are undone together. Later conflicting edits prevent unsafe rollback.",
          )}
        </p>
        {!entries.length && (
          <Message>
            {t(
              "Kein Undo-Verlauf vorhanden oder Undo deaktiviert.",
              "No undo history, or undo is disabled.",
            )}
          </Message>
        )}
        <ol class="history-list">
          {entries.map((entry, index) => (
            <li key={entry.id}>
              <strong>{description(entry.description)}</strong>
              <time dateTime={entry.created_at}>
                {new Date(entry.created_at).toLocaleString()}
              </time>
              <button
                class="text-button"
                disabled={busy || !connection.online}
                onClick={() => prepare(entry.id)}
              >
                {index === 0
                  ? t("Letzte Änderung prüfen", "Review undo of latest change")
                  : `${t("Bis hier zurücknehmen", "Undo through this entry")} (${index + 1})`}
              </button>
            </li>
          ))}
        </ol>
        {preview && (
          <section class="recovery-preview">
            <strong>
              {preview.summary.undo_groups}{" "}
              {t("Änderungsgruppen zurücknehmen", "change groups to undo")}
            </strong>
            <p>
              {preview.summary.affected_tickets.length}{" "}
              {t("betroffene Tickets", "affected tickets")}
            </p>
            <button class="danger" disabled={busy} onClick={execute}>
              {t("Rücknahme bestätigen", "Confirm undo")}
            </button>
          </section>
        )}
      </div>
      <footer class="dialog-footer">
        <button class="secondary" onClick={close}>
          {t("Schließen", "Close")}
        </button>
      </footer>
    </Dialog>
  );
}
