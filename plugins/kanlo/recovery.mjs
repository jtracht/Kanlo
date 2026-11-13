import { randomUUID } from "node:crypto";
import { z } from "zod";

export function registerRecoveryTools(tool, request, id, clientId, writesEnabled) {
  tool("kanlo_list_undo", "Rückgängig-Verlauf eines Boards mit der aktuellen Revision lesen.", {
    board_id: id,
  }, async ({ board_id }) => {
    const { revision } = await request("bootstrap");
    return { revision, items: await request(`undo/${board_id}`) };
  });

  if (!writesEnabled) return;
  const previews = new Map();
  tool("kanlo_preview_undo", "Zeigt die Folgen eines Rückgängig-Vorgangs. Lege die Vorschau dem Nutzer vor und warte auf seine ausdrückliche Bestätigung.", {
    board_id: id,
    revision: z.number().int().nonnegative(),
    through_id: z.number().int().positive().optional(),
  }, async ({ board_id, revision, through_id }) => {
    const preview = await request(`undo/${board_id}`, { revision, through_id });
    for (const [handle, saved] of previews) {
      if (Date.parse(saved.expires_at) <= Date.now()) previews.delete(handle);
    }
    if (previews.size >= 16) previews.delete(previews.keys().next().value);
    const preview_id = randomUUID();
    previews.set(preview_id, preview);
    return { preview_id, expires_at: preview.expires_at, summary: preview.summary };
  }, { readOnlyHint: false });

  tool("kanlo_confirm_undo", "Führt genau die zuvor angezeigte Rückgängig-Vorschau nach ausdrücklicher Nutzerbestätigung aus.", {
    preview_id: z.uuid(),
  }, async ({ preview_id }) => {
    const preview = previews.get(preview_id);
    if (!preview || Date.parse(preview.expires_at) <= Date.now()) {
      return { applied: false, reason: "Die Vorschau ist unbekannt oder abgelaufen. Erneut Vorschau öffnen." };
    }
    previews.delete(preview_id);
    const receipt = await request("recovery/confirm", {
      token: preview.token, op_id: randomUUID(), client_id: clientId,
    });
    return { applied: true, revision: receipt.revision, result: receipt.result };
  }, { readOnlyHint: false, destructiveHint: true });
}
