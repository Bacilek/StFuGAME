# Ovládání bota

## Pro uživatele
- **Zapnout:** dvojklik na zástupce **„StFuGAME bot“** na ploše. U hodin se objeví kulatá ikona a bot hned začne.
  (Když ikonu nevidíš, je možná schovaná pod šipkou ^ v oznamovací oblasti.)
- **Ikona:** zelená = bot běží, šedá = zastavený, červená = skončil chybou (podrobnosti v logu).
- **Pravý klik na ikonu:** Spustit bota / Zastavit bota / Otevřít log / Ukončit.
  - *Zastavit* = bot přestane hrát, ikona zůstane (jde znovu spustit).
  - *Ukončit* = konec programu včetně ikony.
- Když chceš hrát ručně v prohlížeči, nejdřív bota **zastav** (jinak se přetahujete o session).
- Bot jde spustit jen jednou; druhé spuštění ohlásí „Bot už běží“.
- Log: `logs\prubeh.log` (taky přes „Otevřít log“).
- Kdyby ubyly houby mimo povolenou výjimku, bot se sám zastaví a ukáže varování.

## Pro Clauda (vývoj)
- Zástupce spouští `target\release\stfugame.exe` (pracovní složka = projekt). Exe si samo najde složku s `.env`.
- Release build je bez konzole (`windows_subsystem = "windows"`), debug (`cargo run`) má konzoli i ikonu.
- Před `cargo build --release` musí uživatel bota **Ukončit** (exe je jinak zamčené); po buildu ho znovu spustit zástupcem
  (nebo `Start-Process target\release\stfugame.exe -WorkingDirectory <projekt>`).
- Nespouštět `cargo run`, když běží bot z ikony – pojistka jedné instance ukáže okno „Bot už běží“.
