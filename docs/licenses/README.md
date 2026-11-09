# Drittanbieter- und Veröffentlichungsprüfung

`inventory.json` führt die aufgelösten Rust-Pakete sowie die Web- und
Browsertest-Lockdateien mit Version und deklarierter Lizenz auf. Vorhandene
Lizenz-/NOTICE-Dateien sind in den Unterverzeichnissen gesammelt. Optionale,
auf diesem Rechner nicht installierte npm-Plattformpakete sind trotzdem im
Inventar enthalten; fehlende Texte werden durch eine leere `notices`-Liste
sichtbar. Neu erzeugen: `node scripts/license-inventory.mjs` nach `npm ci`.

SQLite wird durch `libsqlite3-sys` eingebunden; dessen mitgelieferte Hinweise
und der Public-Domain-Status der SQLite-Quellen sind zusätzlich zu prüfen.
Keine externe Schrift, kein externes Iconpaket und keine CDN-Ressource wird
zur Laufzeit benötigt. Das Kanlo-Icon ist ein eigenes einfaches SVG.

Vor einer öffentlichen Veröffentlichung erforderlich:

- [ ] Eigentümer gibt konkreten Source-available-Lizenztext frei; der aktuelle
  LICENSE-Platzhalter erteilt keine öffentliche Nutzungslizenz.
- [ ] Alle ausgelieferten Abhängigkeiten und eingebundenen SQLite-Quellen gegen
  den endgültigen Lizenztext prüfen; fehlende Lizenztexte vervollständigen.
- [ ] Produktname „Kanlo“, Logo und mögliche Namensähnlichkeiten durch eine
  geeignete Markenrecherche prüfen; es wird keine Markenfreiheit behauptet.
- [ ] Copyright-Inhaber, Beiträge und Rechte an Bildern/Beispieldaten prüfen.
- [ ] NOTICE und vollständige Drittanbietertexte dem tatsächlichen
  Distributionspaket beilegen; Buildwerkzeuge und Laufzeit getrennt bewerten.
- [ ] Release-, Sicherheits-, Geräte- und Wiederherstellungsabnahmen abschließen.

Das Inventar ersetzt keine rechtliche Freigabe. Kanlo wird nicht als
OSI-Open-Source-Software bezeichnet.
