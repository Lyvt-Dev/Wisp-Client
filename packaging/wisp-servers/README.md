# Wisp Server

In diesem Ordner sind alle Server, die zu Wisp gehören, mit Doppelklick-Dateien zum Einrichten
und Starten. Für den Anfang brauchst du **nur den Hauptserver**.

## Was ist hier drin?

| Datei / Ordner | Wofür |
|---|---|
| **`1 - Einrichten.cmd`** | Einmal am Anfang (und jederzeit wieder): baut die Server-Programme und trägt dich als Owner ein. |
| **`2 - Hauptserver starten.cmd`** | Startet den Hauptserver auf diesem PC mit einer öffentlichen Adresse. |
| **`3 - Creator verwalten.cmd`** | Spieler zum Creator machen, ihren Badge (PNG) setzen, Creator entfernen. |
| `4 - Relay starten (optional).cmd` | Startet das Relay für „Mit Freunden spielen“. |
| `5 - Minecraft-Server (optional).cmd` | Richtet einen eigenen Minecraft-Server ein, der mit Wisp läuft. |
| `Aktualisieren.cmd` | Baut die Server neu aus dem aktuellen Wisp-Code (nach einem Update). |
| `einstellungen.env` | Alle Einstellungen: dein Owner-Account, Ports, wo der Wisp-Code liegt. |
| `daten\` | Die Daten des Hauptservers (`wisp.sqlite3`) und seine Logs. **Davon Backups machen.** |
| `programme\` | Die gebauten Server-Programme. Nicht anfassen, „Aktualisieren“ ersetzt sie. |
| `skripte\` | Die Skripte hinter den Doppelklick-Dateien. |
| `Minecraft-Server\` | Entsteht erst, wenn du `5 - Minecraft-Server` benutzt. |

## Welcher Server ist welcher?

| Server | Programm | Was er macht | Brauchst du ihn? |
|---|---|---|---|
| **Hauptserver** | `wisp-cosmetics-server` | Freunde, Cosmetics und Emotes, die alle sehen, **Wisp-Coins, Shop-Käufe**, Creator, Creator-Codes und Badges. Spieler melden sich über Mojang an (wie bei einem Online-Minecraft-Server), er sieht nie Passwörter. | **Ja.** Ohne ihn gibt es keinen Shop, keine Coins, keine Freunde, keine Creator. |
| Relay | `wisp-relay` | „Mit Freunden spielen“: Einladungscodes, und es leitet die Verbindung weiter, wenn zwei Spieler sich nicht direkt erreichen. | Optional. |
| Minecraft-Server | `wisp-server` | Ein eigener Minecraft-26.3-Server, der mit Wisp statt Java läuft. Hat nichts mit dem Hauptserver zu tun. | Optional. |
| Telemetrie | `wisp-telemetry-server` | Anonyme Nutzungszahlen (nur mit Zustimmung). Ist im Launcher ausgeschaltet. | Nein. |
| Mod-Registry | `wisp-mod-registry` | Mods über GitHub einreichen. | Nein. |

Telemetrie und Mod-Registry sind hier nicht eingebaut, weil Wisp sie nicht braucht. Ihre
Anleitungen liegen im Wisp-Code (`crates/wisp-telemetry-server/README.md`,
`crates/wisp-mod-registry/README.md`).

---

## Schnellstart (auf deinem PC, 3 Schritte)

1. **`1 - Einrichten.cmd`** doppelklicken.
   - Beim ersten Mal installiert es Rust (zum Bauen) und baut die Server. Das dauert ein paar
     Minuten. Windows fragt eventuell einmal nach Erlaubnis.
   - Dann fragt es nach **deinem Minecraft-Namen**. Daraus holt es deine UUID von Mojang und
     trägt dich als Owner ein. **Nur der Owner kann Creator bestimmen.**
2. **`2 - Hauptserver starten.cmd`** doppelklicken.
   - Es startet den Server, installiert beim ersten Mal `cloudflared` (kostenlos) und zeigt
     **„Dein Wisp-Server: https://irgendwas.trycloudflare.com“**.
   - Die Adresse ist schon in die Zwischenablage kopiert und schon in deinem Launcher eingetragen.
3. **Die Adresse an deine Spieler schicken.** Sie tragen sie im Launcher ein:
   **Settings → General → Wisp server**.

Lass das Fenster offen, solange der Server laufen soll. Fenster schließen = Server aus.

> **Achtung:** Die kostenlose Adresse ändert sich bei jedem Start. Dann müssten alle Spieler die
> neue eintragen. Für einen echten Betrieb nimm **[Immer online](#immer-online-feste-adresse)**.

---

## Owner und Creator

**So funktionieren Creator:**

- **Du** (der Owner) bestimmst, wer Creator ist. Niemand sonst kann das.
- Ein Creator legt **im Spiel seinen Creator-Code** fest und kann ihn jederzeit ändern. Seine
  Unterstützer bleiben dabei erhalten.
- Ein Spieler, der den Code eines Creators eingibt, **unterstützt ihn**, bis er einen anderen
  eingibt oder den Code löscht:
  - Er bekommt **10 % Rabatt** auf alles im Shop.
  - Er trägt **den Badge des Creators** vor seinem Namen (statt des Wisp-Badges). Alle
    Wisp-Spieler sehen das.
  - Der Creator bekommt **10 % vom vollen Preis** jedes Kaufs als Coins, direkt in sein Wallet.
- Im Spiel sieht der Creator in seinem Dashboard, wie viele Spieler seinen Code eingetragen
  haben, wie viele Käufe es gab und wie viele Coins er verdient hat.

**Einen Creator anlegen:**

1. Der Spieler muss Wisp **einmal mit deinem Server benutzt** haben (Launcher mit eingetragener
   Adresse öffnen und anmelden reicht).
2. **`3 - Creator verwalten.cmd`** → `1` → seinen Minecraft-Namen eingeben.
3. Seinen Badge setzen: `2` → Name → die PNG-Datei ins Fenster ziehen → Farbe der Linie unter
   dem Namen (z. B. `#ff4d8d`).
   - Der Badge muss eine **PNG mit höchstens 16×16 Pixeln** sein und quadratisch, also
     16×16, 8×8 und so weiter. Transparenz geht.

Als Owner kannst du Creator auch im Spiel hinzufügen und entfernen (Admin-Bereich). Das kommt
mit dem nächsten Wisp-Update.

---

## Coins und Shop

Coins liegen **auf dem Hauptserver**, nicht auf den PCs der Spieler. Niemand kann sich Coins
geben, indem er eine Datei ändert.

- Neue Spieler bekommen **1.000 Coins** Willkommensgeschenk.
- Jeden Tag gibt es **100 Coins**, wenn sie Wisp öffnen.
- Creator bekommen ihren Anteil an den Käufen ihrer Unterstützer.
- Preise und Items kommen aus dem Wisp-Code. Der Server prüft jeden Kauf selbst.

Coins aus älteren Wisp-Versionen lagen nur auf dem jeweiligen PC und werden nicht übernommen.
Alle starten auf dem Server neu mit dem Willkommensgeschenk.

---

## Immer online (feste Adresse)

Auf deinem PC läuft der Server nur, solange der PC an ist, und die Adresse ändert sich. Für den
echten Betrieb gibt es zwei Wege.

### Weg A: Railway (empfohlen, ca. 10 Minuten, ca. 5 $ im Monat)

1. Auf [railway.com](https://railway.com) mit GitHub anmelden.
2. **New Project → Deploy from GitHub repo →** das Wisp-Repository auswählen.
3. Den neuen Service öffnen → **Settings**:
   - **Config file path**: `crates/wisp-cosmetics-server/railway.json`
4. Rechtsklick auf den Service → **Attach volume** → Mount Path **`/data`**.
   **Wichtig:** Ohne Volume sind bei jedem Update alle Coins, Freunde und Creator weg.
5. **Variables → New Variable**:
   - `WISP_ADMINS` = deine UUID. Steht nach „1 - Einrichten“ in `einstellungen.env`. Oder auf
     [mcuuid.net](https://mcuuid.net) nachsehen.
6. **Settings → Networking → Generate Domain**. Du bekommst eine Adresse wie
   `https://wisp-production.up.railway.app`.
7. Im Browser `https://<deine Adresse>/healthz` öffnen. Steht dort `ok`, läuft er.
8. Diese Adresse tragen du und deine Spieler im Launcher ein (**Settings → General → Wisp
   server**). Sie ändert sich nie mehr.

**Creator auf Railway verwalten:** Service öffnen → oben rechts **⋯ → Shell** (oder lokal mit
der Railway-CLI `railway ssh`), dann:

```sh
wisp-cosmetics-server creator <Name>
wisp-cosmetics-server uncreator <Name>
```

Für Badges lädst du die PNG am einfachsten als Base64 über das Spiel hoch (Admin-Bereich, kommt
mit dem nächsten Update). Oder du legst sie vorher ins Volume und rufst
`wisp-cosmetics-server badge <Name> /data/badge.png #ff4d8d` auf.

Jeder Push ins Repository baut den Server auf Railway automatisch neu.

### Weg B: Eigener Linux-Server (ein Befehl)

Auf einem Linux-Server mit Docker und einer Domain, die auf ihn zeigt (A-Record):

```sh
git clone <Wisp-Repository> && cd Wisp-Client
DOMAIN=wisp.deinedomain.de WISP_ADMINS=<deine UUID> docker compose -f crates/wisp-cosmetics-server/docker-compose.yml up -d
```

Damit läuft er mit HTTPS unter `https://wisp.deinedomain.de`. Creator verwaltest du mit
`docker compose -f crates/wisp-cosmetics-server/docker-compose.yml exec wisp wisp-cosmetics-server creator <Name>`.

---

## Relay (optional)

Für „Mit Freunden spielen“ über Einladungscodes.

- **Auf deinem PC:** `4 - Relay starten (optional).cmd`. Spieler außerhalb deines Netzwerks
  erreichen es nur, wenn im Router **TCP-Port 7000** auf diesen PC weitergeleitet ist.
- **Immer online:** Das Relay hat ein eigenes Dockerfile (`crates/wisp-relay/Dockerfile`) und eine
  systemd-Datei (`crates/wisp-relay/wisp-relay.service`). Auf Railway: neuer Service aus demselben
  Repository mit diesem Dockerfile, dann unter **Networking** einen **TCP Proxy** für Port 7000
  anlegen.
- Details: `docs/play-with-friends.md` im Wisp-Code.

## Minecraft-Server (optional)

`5 - Minecraft-Server (optional).cmd` installiert `wisp-server`, stellt dir die Fragen für die
Welt (Name, Seed, Port, Whitelist, Operatoren) und startet ihn im Ordner `Minecraft-Server\`.
Das ist ein normaler Minecraft-26.3-Server, mit dem sich jeder verbinden kann. Er läuft nur mit
Wisp statt Java. Die ganze Anleitung steht in `docs/hosting.md` im Wisp-Code.

---

## Aktualisieren

Nach einem neuen Wisp-Update:

1. Den Wisp-Code aktualisieren (z. B. `git pull` im Wisp-Ordner).
2. **`Aktualisieren.cmd`** doppelklicken.
3. Laufende Server schließen und neu starten.

Auf Railway passiert das bei jedem Push von selbst.

## Backup

Alles Wichtige steckt in **`daten\wisp.sqlite3`**: Coins, gekaufte Items, Freunde, Creator.
Kopier die Datei regelmäßig an einen sicheren Ort, am besten während der Server aus ist. Zum
Wiederherstellen legst du sie einfach zurück.

## Wenn etwas nicht geht

| Problem | Lösung |
|---|---|
| „Der Hauptserver ist noch nicht gebaut“ | Zuerst `1 - Einrichten.cmd`. |
| „Der Wisp-Code wurde nicht gefunden“ | In `einstellungen.env` bei `WISP_REPO=` den Ordner des Wisp-Codes eintragen. |
| Server startet nicht | Log ansehen: `daten\server.log`. Oft ist Port 8080 belegt, dann in `einstellungen.env` z. B. `PORT=8090`. |
| Keine öffentliche Adresse | `cloudflared` wurde blockiert. Im selben Netzwerk geht die angezeigte lokale Adresse. Sonst Railway nehmen. |
| Creator anlegen: „hasn't used Wisp yet“ | Der Spieler muss sich einmal mit deinem Server angemeldet haben (Launcher mit deiner Server-Adresse öffnen). |
| Badge wird abgelehnt | PNG größer als 16×16 oder nicht quadratisch. |
| Spieler sehen nichts | Prüfen, dass bei ihnen im Launcher unter Settings → General → Wisp server **genau deine Adresse** steht. |
| Du kannst keine Creator bestimmen | In `einstellungen.env` muss bei `WISP_ADMINS` deine UUID stehen (auf Railway als Variable). Danach den Server neu starten. |
