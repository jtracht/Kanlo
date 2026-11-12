import { useState } from "preact/hooks";
import { api } from "./client";
import { Message, useT } from "./ui";

interface Status {
  version: string;
  manual_updater_installed: boolean;
}

export function UpdateStatus() {
  const t = useT();
  const [status, setStatus] = useState<Status>();
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  async function readStatus() {
    setPending(true);
    setFailed(false);
    try {
      setStatus(await api<Status>("/system/update"));
    } catch {
      setStatus(undefined);
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  return (
    <details>
      <summary>{t("Manuelles Server-Update", "Manual server update")}</summary>
      <button type="button" class="secondary" disabled={pending} onClick={() => void readStatus()}>
        {t("Installierten Stand anzeigen", "Show installed version")}
      </button>
      {failed && <Message error>{t("Serverstatus konnte nicht gelesen werden.", "Could not read the server status.")}</Message>}
      {status && (
        <>
          <p>{t("Installierte Version", "Installed version")} {status.version}</p>
          <p>
            {status.manual_updater_installed
              ? t("Der Geräteadministrator kann ein geprüftes Serverpaket per Terminal einspielen. Die Schritte stehen in INSTALL-DE.md im Serverpaket.", "The device administrator can apply a verified package in a terminal. See INSTALL-EN.md in the server package.")
              : t("Der manuelle Updater gehört zum installierten Linux-Paket.", "The manual updater is included in the installed Linux package.")}
          </p>
        </>
      )}
    </details>
  );
}
