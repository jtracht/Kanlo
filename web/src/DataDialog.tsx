import { useState } from "preact/hooks";
import type { Snapshot, Receipt } from "./types";
import { api, connection, loadBootstrap, loadBoard } from "./client";
import { Dialog, Message, useT, errorText } from "./ui";
import * as storage from "./storage";
import { uuid } from "./identifiers";

function download(value: string, filename: string, type = "application/json") {
  const url = URL.createObjectURL(new Blob([value], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 30000);
}
interface Package {
  schema_version: number;
  workspace: {
    boards: { id: string; name: string; prefix: string }[];
    tickets: unknown[];
  };
  [key: string]: unknown;
}
interface Preview {
  token: string;
  summary: Record<string, unknown>;
  purpose: string;
}
export function DataDialog({
  snapshot,
  close,
  refresh,
}: {
  snapshot: Snapshot;
  close: () => void;
  refresh: () => void;
}) {
  const t = useT();
  const board = snapshot.workspace.boards[0];
  const [file, setFile] = useState<{
    name: string;
    content: string;
    csv: boolean;
  }>();
  const [source, setSource] = useState<Package>();
  const [mode, setMode] = useState("skip");
  const [prefixes, setPrefixes] = useState<Record<string, string>>({});
  const [restore, setRestore] = useState(false);
  const [selective, setSelective] = useState(false);
  const [selected, setSelected] = useState<string[]>([]);
  const [preview, setPreview] = useState<Preview>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [operation, setOperation] = useState(uuid());
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function read(file: File) {
    await run(async () => {
      if (file.size > 32 * 1024 * 1024)
        throw new Error(
          t("Datei ist größer als 32 MiB.", "File exceeds 32 MiB."),
        );
      const content = await file.text();
      const csv = file.name.toLowerCase().endsWith(".csv");
      setFile({ name: file.name, content, csv });
      setPreview(undefined);
      setMessage("");
      if (csv) {
        setSource(undefined);
        setRestore(false);
        return;
      }
      const source = JSON.parse(content) as Package;
      if (!source.workspace?.boards || !Array.isArray(source.workspace.tickets))
        throw new Error(
          t("Keine Kanlo-Datendatei.", "Not a Kanlo data package."),
        );
      setSource(source);
      setRestore("history" in source && "statistics" in source);
      setSelected(
        source.workspace.boards
          .filter((value) => value.id === board.id)
          .map((value) => value.id),
      );
      setPrefixes(
        Object.fromEntries(
          source.workspace.boards.map((board) => [
            board.id,
            `${board.prefix.slice(0, 8)}COPY`,
          ]),
        ),
      );
    });
  }
  async function prepare() {
    if (!file) return;
    await run(async () => {
      setOperation(uuid());
      if (file.csv)
        setPreview(
          await api("/imports/csv/preview", {
            board_id: board.id,
            csv: file.content,
            mode,
          }),
        );
      else if (restore)
        setPreview(
          await api("/backups/restore/preview", {
            backup: source,
            boards: selective ? selected : null,
          }),
        );
      else
        setPreview(
          await api("/imports/preview", {
            package: source,
            mode,
            prefix_overrides: mode === "new" ? prefixes : {},
          }),
        );
    });
  }
  async function execute() {
    if (!preview) return;
    await run(async () => {
      const receipt = await api<Receipt>("/recovery/confirm", {
        token: preview.token,
        op_id: operation,
        client_id: await storage.clientId(),
      });
      await storage.acknowledge(
        {
          op_id: receipt.op_id,
          endpoint: "/recovery/confirm",
          body: {},
          action: { kind: preview.purpose },
          board_id: board.id,
          created_at: new Date().toISOString(),
          state: "pending",
        },
        {},
        receipt,
      );
      setPreview(undefined);
      setFile(undefined);
      setSource(undefined);
      setMessage(
        t(
          "Änderung auf dem Server ausgeführt. Lokale Warteschlangen bleiben zur Prüfung erhalten.",
          "Server operation completed. Local queues are retained for review.",
        ),
      );
      await loadBootstrap();
      await loadBoard(board.id).catch(() => {});
      refresh();
    });
  }
  return (
    <Dialog
      title={t("Daten und Sicherung", "Data and backups")}
      close={close}
      wide
    >
      <div class="dialog-body">
        {error && <Message error>{error}</Message>}
        {message && <Message>{message}</Message>}
        <section>
          <h3>
            {t("Aktuelles Board exportieren", "Export current board")} ·{" "}
            {board.name}
          </h3>
          <p>
            {t(
              "JSON enthält Spalten, Tickets, Untertickets, Kommentare, Textfelder, Labels, Typen, Links und Abhängigkeiten. Der Undo-Verlauf gehört nicht zum Boardexport.",
              "JSON includes columns, tickets, subtickets, comments, custom fields, labels, types, links and dependencies. Board exports exclude undo history.",
            )}
          </p>
          <div class="actions wrap">
            <button
              class="secondary"
              disabled={busy || !connection.online}
              onClick={() =>
                run(async () =>
                  download(
                    JSON.stringify(await api(`/exports/${board.id}`), null, 2),
                    `kanlo-${board.prefix}.json`,
                  ),
                )
              }
            >
              {t("Board als JSON", "Board as JSON")}
            </button>
            <button
              class="secondary"
              disabled={busy || !connection.online}
              onClick={() =>
                run(async () => {
                  const response = await fetch(
                    `/api/v1/exports/${board.id}/csv`,
                    { credentials: "same-origin" },
                  );
                  if (!response.ok) throw new Error(`HTTP ${response.status}`);
                  download(
                    await response.text(),
                    `kanlo-${board.prefix}.csv`,
                    "text/csv",
                  );
                })
              }
            >
              {t("Tabellarisches CSV", "Tabular CSV")}
            </button>
          </div>
          <small>
            {t(
              "CSV enthält Kennung, Titel, Beschreibung, Priorität, Deadline und Spaltenname; keine komplexen Beziehungen. Formeln werden entschärft.",
              "CSV includes ID, title, description, importance, deadline and column name; not complex relationships. Formula cells are neutralized.",
            )}
          </small>
        </section>
        <section>
          <h3>{t("Vollständige Datensicherung", "Complete data backup")}</h3>
          <p>
            {t(
              "Konsistenter Datenstand einschließlich Verlauf und Statistik; unverschlüsselt und ohne Zugangscode, API-Schlüssel oder Sitzungstokens.",
              "Consistent data snapshot including history and statistics; unencrypted and excluding passcodes, API keys and session tokens.",
            )}
          </p>
          <button
            class="secondary"
            disabled={busy || !connection.online}
            onClick={() =>
              run(async () =>
                download(
                  JSON.stringify(await api("/backups", {}), null, 2),
                  `kanlo-backup-${new Date().toISOString().slice(0, 10)}.json`,
                ),
              )
            }
          >
            {t("Sicherung herunterladen", "Download backup")}
          </button>
        </section>
        <section>
          <h3>{t("Importieren oder wiederherstellen", "Import or restore")}</h3>
          <label>
            {t("Kanlo-JSON oder CSV auswählen", "Choose Kanlo JSON or CSV")}
            <input
              type="file"
              accept=".json,.csv,application/json,text/csv"
              onChange={(event) => {
                const file = event.currentTarget.files?.[0];
                if (file) void read(file);
              }}
            />
          </label>
          {file && (
            <>
              <p>
                {file.name}
                {source &&
                  ` · ${source.workspace.boards.length} ${t("Boards", "boards")} · ${source.workspace.tickets.length} ${t("Tickets", "tickets")}`}
              </p>
              {restore ? (
                <>
                  <Message>
                    {t(
                      "Wiederherstellung ändert die Datenepoche. Vor dem Überschreiben wird der aktuelle Datenstand zusätzlich gesichert. API-Schlüssel und externe Sitzungen werden ungültig.",
                      "Restore changes the data epoch and creates an additional safety snapshot before overwriting. API keys and external sessions are invalidated.",
                    )}
                  </Message>
                  <label class="inline check">
                    <input
                      type="checkbox"
                      checked={selective}
                      onChange={(event) => {
                        setSelective(event.currentTarget.checked);
                        setPreview(undefined);
                      }}
                    />
                    {t(
                      "Nur ausgewählte Boards wiederherstellen",
                      "Restore selected boards only",
                    )}
                  </label>
                  {selective && (
                    <fieldset>
                      <legend>
                        {t("Zu ersetzende Boards", "Boards to replace")}
                      </legend>
                      {source?.workspace.boards.map((board) => (
                        <label class="inline check" key={board.id}>
                          <input
                            type="checkbox"
                            checked={selected.includes(board.id)}
                            onChange={(event) => {
                              setSelected(
                                event.currentTarget.checked
                                  ? [...selected, board.id]
                                  : selected.filter((id) => id !== board.id),
                              );
                              setPreview(undefined);
                            }}
                          />
                          {board.name} ({board.prefix})
                        </label>
                      ))}
                    </fieldset>
                  )}
                </>
              ) : (
                <>
                  <fieldset>
                    <legend>
                      {t("Bei passenden Kennungen", "When identifiers match")}
                    </legend>
                    {[
                      ["skip", t("Überspringen (Standard)", "Skip (default)")],
                      [
                        "update",
                        t("Ausdrücklich aktualisieren", "Explicitly update"),
                      ],
                      [
                        "new",
                        t(
                          "Als neue Tickets importieren",
                          "Import as new tickets",
                        ),
                      ],
                    ].map(([value, label]) => (
                      <label class="inline check" key={value}>
                        <input
                          type="radio"
                          name="import-mode"
                          checked={mode === value}
                          onChange={() => {
                            setMode(value);
                            setPreview(undefined);
                          }}
                        />
                        {label}
                      </label>
                    ))}
                  </fieldset>
                  {mode === "new" &&
                    source?.workspace.boards.map((board) => (
                      <label key={board.id}>
                        {t("Neues Boardkürzel für", "New board prefix for")}{" "}
                        {board.prefix}
                        <input
                          pattern="[A-Z]{1,12}"
                          maxLength={12}
                          value={prefixes[board.id] ?? ""}
                          onInput={(event) => {
                            setPrefixes({
                              ...prefixes,
                              [board.id]:
                                event.currentTarget.value.toUpperCase(),
                            });
                            setPreview(undefined);
                          }}
                        />
                      </label>
                    ))}
                </>
              )}
              <button
                class="secondary"
                disabled={
                  busy ||
                  !connection.online ||
                  (restore && selective && !selected.length)
                }
                onClick={prepare}
              >
                {t("Vorschau erstellen", "Create preview")}
              </button>
            </>
          )}
          {preview && (
            <section class="recovery-preview">
              <h4>
                {t(
                  "Vorschau – noch nicht ausgeführt",
                  "Preview — not executed yet",
                )}
              </h4>
              <dl>
                {Object.entries(preview.summary)
                  .filter(
                    ([, value]) =>
                      typeof value === "number" ||
                      typeof value === "boolean" ||
                      typeof value === "string",
                  )
                  .map(([key, value]) => (
                    <div key={key}>
                      <dt>{summaryName(key, t)}</dt>
                      <dd>{String(value)}</dd>
                    </div>
                  ))}
              </dl>
              <details>
                <summary>
                  {t(
                    "Vollständige Zuordnung prüfen",
                    "Inspect complete mapping",
                  )}
                </summary>
                <pre class="data-preview">
                  {JSON.stringify(preview.summary, null, 2)}
                </pre>
              </details>
              <button
                class="danger"
                disabled={busy || !connection.online}
                onClick={execute}
              >
                {t(
                  "Vorschau bestätigen und ausführen",
                  "Confirm preview and execute",
                )}
              </button>
            </section>
          )}
        </section>
      </div>
      <footer class="dialog-footer">
        <button class="secondary" onClick={close}>
          {t("Schließen", "Close")}
        </button>
      </footer>
    </Dialog>
  );
}
function summaryName(key: string, t: (de: string, en: string) => string) {
  const names: Record<string, [string, string]> = {
    created: ["Neue Tickets", "New tickets"],
    updated: ["Aktualisierte Tickets", "Updated tickets"],
    skipped: ["Übersprungene Tickets", "Skipped tickets"],
    boards: ["Boards im Ergebnis", "Resulting boards"],
    tickets: ["Tickets im Ergebnis", "Resulting tickets"],
    epoch_will_change: [
      "Datenstand wird neu gekennzeichnet",
      "Data epoch will change",
    ],
    safety_backup_before_overwrite: [
      "Zusätzliche Sicherung vor Überschreiben",
      "Safety backup before overwrite",
    ],
    mode: ["Importmodus", "Import mode"],
  };
  return names[key] ? t(...names[key]) : key;
}
