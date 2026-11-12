"""Write a reproducible CSV workload for a disposable Kanlo test board."""
import argparse
import csv
from pathlib import Path

TICKET_COUNT = 1200
DETAILS = """## Prüfschritte

1. Ticket öffnen und zum Ende des Inhalts scrollen.
2. Zwischen den zwei geöffneten Tickets wechseln.
3. Zur Übersicht zurückkehren.

| Ausgabe | Prüfung |
| --- | --- |
| HDMI | Board und Tickettext sichtbar |
| Composite | Text und Spalten sichtbar |

```text
Board → Ticket → Backlog
```
"""


def write_workload(destination, columns):
    with destination.open('x', encoding='utf-8', newline='') as output:
        writer = csv.writer(output)
        writer.writerow(('title', 'description', 'importance', 'column_name'))
        for index in range(TICKET_COUNT):
            title = f'Geräteabnahme {index + 1:04}'
            description = '\n'.join(
                f'# Abschnitt {section + 1}\n\n{DETAILS}' for section in range(8)
            ) if index < 2 else 'Reproduzierbares Ticket für den Dauerlasttest.'
            column = '' if index % 4 == 3 else columns[index % 4]
            writer.writerow((title, description, (index * 7) % 101, column))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--columns', nargs=3, required=True, metavar=('FIRST', 'SECOND', 'THIRD'))
    args = parser.parse_args()
    if len(set(args.columns)) != 3 or any(not column.strip() for column in args.columns):
        parser.error('Select three distinct, nonempty column names')
    try:
        write_workload(args.destination, args.columns)
    except FileExistsError:
        parser.error(f'{args.destination} already exists')
