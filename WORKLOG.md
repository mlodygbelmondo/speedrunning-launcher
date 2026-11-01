# Work log

Co jest zrobione, co jest w toku, na co czekamy i co już zdecydowaliśmy. Aktualizuję ten plik w każdym
commicie, który coś zmienia. Najnowsze wpisy dziennika są na górze.

## Teraz

- **Pętla ulepszeń (od 2026-11-01, prośba Piotra):** pomysł → zrobienie → przegląd → poprawki, aż każda z 10 miar
  ma co najmniej 9/10. Oceny po każdej rundzie:

  | Miara | Start | Teraz |
  |---|---|---|
  | 1. Wygląd i dopracowanie | 7,5 | 8 |
  | 2. Ruch i płynność (animacje) | 5 | 8 |
  | 3. Model instancji (własne instancje: tworzenie, kopia, usuwanie) | 3 | 9 |
  | 4. Zawartość instancji (światy, mody, paczki, konfiguracja, logi) | 4 | 9 |
  | 5. Nawigacja i układ informacji | 7 | 8,5 |
  | 6. Teksty EN i PL | 7 | 8 |
  | 7. Odporność i bezpieczeństwo danych | 7,5 | 9 |
  | 8. Wydajność (bezczynność 0 klatek, start) | 9 | 9 |
  | 9. Jakość kodu i testy | 8 | 8,5 |
  | 10. Wartość dla speedrunnera (legalność, narzędzia, trening, staty) | 8 | 9 |

- **Stan:** `launcher` i `main` to ten sam commit, a pierwsze numerowane wydanie
  `v0.1.0` jest opublikowane (2026-10-31, niepodpisane, `releases/latest`). Gotowe są M1, M2 i M3 (statystyki
  z prawdziwych danych, mapy treningowe, logowanie Microsoft z ekranami), import ustawień z innych launcherów,
  jedna instancja 1.16.1 ze stroną Mody, szlif UI i tekstów, blokada grania offline w wersji z client ID,
  podpis SignPath po stronie CI i samoaktualizacja. Szczegóły w dzienniku.
- **Repozytorium jest publiczne** od 2026-10-31 (GPL-3.0-or-later), z jednym commitem na start. Historia sprzed
  niego jest w prywatnym `mlodygbelmondo/speedrunning-launcher-private` (lokalnie remote `private`).
- **Podpis SignPath:** CI jest gotowe (ręczne uruchomienie z zaznaczonym `sign`, a wydania z tagu same, gdy
  SignPath jest skonfigurowany), README ma „Code signing policy”. Czeka na Piotra: wniosek na signpath.org, potem
  konfiguracja z `launcher/tools/windows/SIGNPATH.md`.
- **Samoaktualizacja launchera** jest zrobiona i wypchnięta (2026-10-31, szczegóły w dzienniku), CI zielone,
  wydanie `v0.1.0` opublikowane, suma SHA-256 sprawdzona po pobraniu. Nikt jej jeszcze nie uruchomił na Windowsie.
  Kolejne wydanie: podbij wersję w `launcher/Cargo.toml`, commit, tag `vX.Y.Z`, push tagu (`launcher/README.md`,
  „Releasing a version”). Najnowsze jest `v0.1.4` (Toolscreen wstrzykiwany po otwarciu okna gry). Test na
  Windowsie: Piotr ma 0.1.3, więc launcher ma sam pobrać 0.1.4, a potem Ranked i FSG z Toolscreen mają dojść do
  ekranu tytułowego.
- **Logowanie Microsoft** jest wyłączone, dopóki Mojang nie zatwierdzi naszego client ID. Poradnik dla Piotra:
  `launcher/docs/microsoft-login.md` (Azure i formularz Mojanga, po polsku). Bez client ID gra offline działa jak
  dotąd, więc testy w czasie weryfikacji nie potrzebują konta.
- **Czeka na Piotra:** test paczki na prawdziwym Windowsie 10/11 x64. Pobierz
  <https://github.com/mlodygbelmondo/speedrunning-launcher/releases/download/launcher-latest/speedrunning-launcher-windows.zip>,
  rozpakuj i uruchom `speedrunning-launcher.exe`. Przy pierwszym starcie wpisz nick i, jeśli masz MultiMC, Prism
  albo MCSR Launcher, spróbuj „Importuj moje ustawienia”. Potem wybierz RSG i kliknij Pobierz: gra powinna dojść do
  ekranu tytułowego z modami. Najważniejsze są Toolscreen, czyste zatrzymanie gry i import, bo tego nie da się
  sprawdzić na Linuksie:
  1. Import: launcher znajduje Twoje instancje (MultiMC, Prism, MCSR Launcher, `.minecraft`), a po imporcie gra ma
     Twoje klawisze, czułość i paczki zasobów. Kopie zapasowe są w `import-backup-<czas>` w folderze instancji.
  2. Narzędzia: Toolscreen po instalacji pokazuje 1.4.7, a w `%APPDATA%\SpeedrunningLauncher\data\tools\toolscreen\1.4.7\x64\`
     są `Toolscreen.dll` i `liblogger_x64.dll`.
  3. Graj w RSG: okienko Toolscreen pojawia się w lewym górnym rogu gry, Ctrl+I otwiera jego ustawienia, a Thin,
     Wide i EyeZoom przełączają się od razu.
  4. Plik `%USERPROFILE%\.config\toolscreen\logs\latest.log` istnieje i ma linie startu Toolscreen.
  5. Windows Defender ani inny antywirus nie blokuje launchera przy wstrzykiwaniu DLL. Jeśli blokuje, zapisz, co
     pokazał.
  6. Ninjabrain Bot otwiera się z grą, bez okna konsoli, i zamyka się po wyjściu z gry. Jeśli uruchomisz go sam
     wcześniej, launcher go zostawia.
  7. Zatrzymaj grę z launchera: gra zamyka się jak po kliknięciu X, a jej log kończy się na `Stopping!`. Gra nie
     może zostać zabita.
  8. Po RSG zagraj w SSG albo FSG: to ta sama instancja (`instances\mcsr`), więc tryb dokłada tylko swoje mody,
     a klawisze i światy zostają. Seed wpisany w Atum w SSG nie może się pojawić po powrocie do RSG (ani seed
     z zaimportowanej instancji SSG). W Trybach przycisk Mody pozwala włączyć i wyłączyć mod, a „Dodaj własny
     .jar…” otwiera okno wyboru pliku Windowsa; jar Fabric API launcher odrzuca.
  9. Statystyki po kilku resetach pokazują te runy. Trening: zainstaluj jedną mapę (np. Bastion Trainer) i zagraj.
  10. Ustawienia, potem Autorzy i licencje: w kolumnie Kod są Pandora Launcher i EasyInjectBundled.

  W Ranked mod MCSR Ranked najpierw pokazuje ekran zgody na politykę prywatności i zasady. Akceptujesz go sam,
  w grze.
- **Czego jeszcze brakuje:**
  - Nikt nie uruchomił paczki na pulpicie Windowsa. CI instaluje na Windowsie bez okna RSG, Ninjabrain Bota
    i Toolscreen. Wstrzyknięcie Toolscreen było sprawdzone tylko pod wine (testowa DLL w notatniku), nigdy w grze.
  - waywall nie był uruchomiony z grą, bo test E2E działa w Xvfb, a nie w sesji Wayland.
  - Gra uruchomiona w poprzedniej sesji launchera nie jest pokazywana jako trwająca.
  - Toolscreen nie jest jeszcze przygotowany dla Draftout (26.3 potrzebuje jego warstwy Vulkan).
  - Gdy mcsr-meta nie ma legalnej wersji moda dla trybu, tryb daje błąd. Nie wraca do starszej legalnej wersji.
  - Przerwana aktualizacja Javy bez sieci: launcher nie wraca jeszcze sam do poprzedniej wersji.
  - Katalog danych na Windowsie: Roaming czy LocalAppData, do decyzji.
  - Samoaktualizacja tylko na Linuksie i w testach: zmiana nazwy działającego exe, start nowego i usunięcie
    `.old.exe` przy następnym starcie trzeba sprawdzić na prawdziwym Windowsie (też z Defenderem i folderem
    w Program Files, gdzie launcher ma pokazać link do wydań).
  - Listę modów Draftout trzeba sprawdzać ręcznie przy każdej nowej wersji Draftout.
  - Ekrany M3 bez pełnego przeglądu kodu (29.10 przejrzany był backend: wspólna instancja, import, trening, pod
    kątem utraty danych i Windowsa; wyniki w dzienniku). Niesprawdzone końcówki: kod z telefonu i Anuluj
    w logowaniu, ostrzeżenie „ważne do” w wierszu treningu, ponowienie po ponownym zalogowaniu, Aktualizuj
    i Anuluj mapy, elo Draftout na prawdziwych danych.
  - Wspólna instancja: niesprawdzona na Windowsie (jary kopiowane, nie linkowane), z prawdziwym oknem wyboru pliku
    i w prawdziwym meczu Ranked.
  - Z oceny ekranów zostały decyzje Piotra: Tryby względem Startu i własna grafika map (system przycisków
    i typografii jest na gałęzi `design-system`, czeka na scalenie).

## Plan

### Etap 1: badania i bake-off (zrobione)

- [x] Raporty w `research/`: stack, ekosystem MCSR, logowanie i rdzeń launchera, mapy treningowe, kierunki wizualne.
- [x] Bake-off Tauri, Electron i GPUI z tą samą sceną i harnessem pomiarowym (Linux i Windows).
- [x] Prototyp wszystkich ekranów (scene v2), EN i PL z przełącznikiem.
- [x] Dwa przebiegi na Windowsie (`bakeoff/results/windows-*`).

### Etap 2: redesign prototypu, scene v3 (wzór dla aplikacji)

- [x] Shell: górna nawigacja zamiast bocznej szyny.
- [x] Start, w 5 stanach trybów.
- [x] Tryby.
- [x] Trening.
- [x] Własne rendery z Blendera, 11 obrazów, struktury z prawdziwego świata.
- [x] Poprawka migania tła w GPUI.

Pozostałe ekrany powstają od razu w prawdziwej aplikacji (etap 4), już tylko w GPUI.

### Etap 3: decyzje (zrobione 2026-10-26, poza logowaniem)

- [x] Prawdziwy launcher zamiast dalszego prototypu.
- [x] Stack: GPUI.
- [x] Strategia: własna aplikacja i moduły Pandory (MIT).
- [ ] Logowanie Microsoft: na razie konto offline. Własny client ID u Mojang później.
- [ ] Legalizacja launchera na Discordzie MCSR (speedrun.com, reguła A.1.1), gdy będzie działająca wersja.
- [ ] Licencja kodu (przed publikacją).

### Etap 4: prawdziwy launcher, gałąź `launcher`, katalog `launcher/`

Architektura i zasady są w `launcher/README.md`.

M1: wybierasz tryb, klikasz Graj i gra startuje. Strumienie pracy:

- [x] Szkielet: workspace, wspólne typy w `srl-core` (ścieżki, tryby, postęp, konto offline, pobieranie
  z weryfikacją hashy).
- [x] `srl-game`: Minecraft, biblioteki, assety, Java, Fabric, start gry.
- [x] `srl-mods`: legalne mody z mcsr-meta, sprawdzane z `legal-builds.csv`, zestawy dla trybów.
- [x] `srl-app`: UI v3 przeniesione z prototypu, prawdziwe stany przycisku Graj.
- [x] Integracja: Graj naprawdę instaluje i odpala grę, test od zera na Linuksie, CI z paczką na Windowsa.
- [x] Przegląd kodu i poprawki.

M2: narzędzia i reszta ekranów. Strumienie pracy:

- [x] `srl-tools`: Ninjabrain Bot, Toolscreen (Windows) i waywall (Linux), z przypiętymi legalnymi wersjami, razem
  z grą. `srl-game` zatrzymuje grę tak, żeby zapisała świat.
- [x] Ekrany Ustawienia i Narzędzia, przetłumaczone błędy startu gry.
- [x] Ekrany Nauka i Statystyki.
- [x] Integracja: Narzędzia i Graj na prawdziwym `srl-tools`, nowe ustawienia w `srl-mods` i `srl-game`, test od
  zera na Linuksie, CI instaluje narzędzia na Windowsie.
- [x] Przegląd kodu M2 i poprawki.
- [ ] Test na pulpicie Windowsa (Piotr, lista w „Teraz”).

M3: statystyki z prawdziwych danych, mapy treningowe i logowanie Microsoft. Strumienie pracy:

- [x] `srl-stats`: rekordy SpeedRunIGT (PB, splity, suma najlepszych odcinków, liczba runów na tryb), API MCSR Ranked
  i API Draftout, z kopią na dysku na czas bez sieci.
- [x] `srl-practice`: katalog 13 map do pobrania od autorów (sha512 przypięte), instancja treningowa, instalacja,
  reset, aktualizacja i usuwanie map.
- [x] `srl-auth`: logowanie Microsoft (przeglądarka z PKCE i kod z telefonu), tokeny w systemowym magazynie haseł.
  Wyłączone bez client ID.
- [x] Przegląd kodu M3 (rdzeń) i poprawki, scalenie do `launcher`.
- [x] Ekrany: Statystyki, konto Microsoft w Ustawieniach i w Graj, Trening (`launcher/docs/m3-ui-plan.md`), 38c66c1.
- [x] Granie offline w wersji z client ID dopiero po pierwszym zalogowaniu (decyzja Piotra).
- [ ] Przegląd kodu ekranów M3 (pominięty, żeby oszczędzić limit).
- [ ] Client ID od Mojanga (Piotr, `launcher/docs/microsoft-login.md`).

Poza planem, na prośbę Piotra:

- [x] Import ustawień z MultiMC, Prism, MCSR Launchera i `.minecraft` (f8733a3).
- [x] Jedna instancja 1.16.1 dla Ranked, RSG, FSG i SSG oraz strona Mody (ccf3655).
- [x] Ocena ekranów (wygląd 6,5, teksty EN 6, PL 5,5, „nie vibecoded” 5,5) i 14 szybkich poprawek (06aa624).
- [x] Większa praca z oceny: system przycisków i typografii, przewijane listy, „O mapie” w Treningu, przełączniki
  w Narzędziach (gałąź `design-system`).
- [ ] Z oceny, do decyzji Piotra: Tryby względem Startu, własna grafika map i renderu Ranked.

Potem:

- [x] Aktualizacje launchera: numerowane wydania z tagów i samoaktualizacja (2026-10-31, czeka na test na Windowsie).
- [ ] Instalator i podpis.

## Decyzje

- **2026-10-31**, Piotr: repozytorium publiczne na licencji GPL-3.0-or-later, jako nowe repo z czystą historią;
  stara historia zostaje prywatna. Podpis exe przez SignPath Foundation (darmowy dla open source).
- **2026-10-29**, Piotr: konta offline jak w Prism. Wersja z client ID pozwala grać offline dopiero po jednym
  zalogowaniu kontem Microsoft z grą na tym komputerze (wylogowanie tego nie cofa); wcześniej Graj prosi
  o zalogowanie. Wersja bez client ID gra offline jak dotąd, więc testy w czasie weryfikacji Mojanga działają bez
  konta. Limit CPU dla agentów zostaje wyłączony.
- 2026-10-26, Piotr: jedna instancja dla Ranked, RSG, FSG i SSG, a mody włączane i wyłączane jak w MultiMC (nie
  osobna instancja na tryb). Wygląd launchera bez zmian: karta trybu to gotowy zestaw modów. Nielegalny zestaw:
  ostrzeżenie, gra startuje. Na liście są wszystkie legalne mody i dodatki plus własne jary. Wybór seeda w SSG
  na razie bez zmian.
- **2026-10-26:** rdzeń M3 (statystyki, mapy treningowe, logowanie Microsoft) powstaje bez ekranów, równolegle
  z końcówką M2. Agenci budujący pracują na wysiłku high, przeglądy na xhigh. Client ID Microsoftu nigdy nie trafia
  do kodu: launcher czyta go z `SRL_MSA_CLIENT_ID`, a bez niego logowanie jest wyłączone.
- **2026-10-26:** nowa nazwa to Speedrunning Launcher (wcześniej Speedrunluncher): repozytorium
  `speedrunning-launcher`, plik `speedrunning-launcher.exe`, paczka `speedrunning-launcher-windows.zip`, dane
  w `SpeedrunningLauncher`. Dane spod starej nazwy launcher przenosi sam przy pierwszym starcie. Crate'y zostają
  z prefiksem `srl-`.
- **2026-10-26:** narzędzie, które nie może ruszyć, nigdy nie blokuje gry. Launcher je pomija i mówi dlaczego na
  ekranie Gry. waywall launcher tylko znajduje w systemie, nie instaluje go. Ninjabrain Bot nie startuje w Draftout,
  bo jego zasady go zabraniają.
- **2026-10-26:** M2 to narzędzia i reszta ekranów (Nauka, Narzędzia, Statystyki, pełne Ustawienia).
  Opcjonalne dodatki Draftout (No Chat Restrictions, No Telemetry) są domyślnie wyłączone, z przełącznikiem
  w Ustawieniach.
- **2026-10-26:** w Draftout instalujemy tylko mody, które przepuszcza sam mod Draftout (jego lista z wersji
  1.18.0). Planifolia i SpeedRunIGT są w mcsr-meta legalne dla 26.3, ale z nimi Draftout zatrzymuje start gry. Listę
  trzeba sprawdzać przy każdej nowej wersji Draftout.
- **2026-10-26:** okno i tryb bez okna (`speedrunluncher --install <tryb>`) używają tej samej ścieżki Graj. CI
  instaluje nią RSG na Windowsie przy każdym pushu.
- **2026-10-26:** przy starcie stan trybów bierze się z zapisu ostatniej instalacji i z plików na dysku, bez sieci.
  Aktualizacje launcher sprawdza w tle. Symulowany backend zostaje tylko do zrzutów ekranu (`SRL_FAKE=1`).
- **2026-10-26:** launcher nie akceptuje za gracza zgody MCSR Ranked, bo to decyzja gracza.
- **2026-10-26:** paczkę `launcher-latest` podmieniamy tylko wtedy, gdy CI przejdzie na Linuksie i na Windowsie.
- **2026-10-26:** budujemy prawdziwy launcher w GPUI: własna aplikacja z modułami Pandory (MIT). Na razie
  bez logowania Microsoft, na koncie offline.
- **2026-10-26:** agenci pracują w osobnych worktree, razem najwyżej na 8 z 16 wątków i 12 GB RAM (slice
  `srl.slice`, polecenie `srl-cpu`), bo Piotr pracuje równolegle nad innym projektem.
- **2026-10-26:** poprawka migania w GPUI bez wstępnego ładowania obrazów, bo to kosztowało około 45 MB pamięci.
- **2026-10-25:** kierunek A („Plansza”) plus pasek z kierunku B. Własne rendery kluczowych obrazów, bez plików
  Mojang.
- **2026-10-25:** angielski domyślnie, polski jako drugi język, z przełącznikiem w Ustawieniach.
- **2026-10-25:** launcher tylko przygotowuje i uruchamia grę. Kolejki i pokoje Ranked są w grze, więc nie ma
  przycisku „szukaj meczu”.
- **2026-10-24:** bake-off poszerzony o Electron i GPUI. Qt pominięty.
- **2026-10-23:** styl A+D, czyli Violet Noir plus dane speedrunowe.

## Dziennik

### 2026-11-01

- **Podgląd konfiguracji w launcherze:** na karcie Konfiguracja pliki tekstowe (json, txt, toml, properties, cfg,
  yml…) mają „Pokaż”: pierwsze 400 linii pod wierszem, długie linie łamane co 110 znaków (jednowierszowy JSON
  da się czytać); kliknięcie wiersza też pokazuje i chowa. Inne pliki dalej otwierają się w programie systemu.
  Plik nie-UTF-8 dostaje komunikat zamiast śmieci. `content::preview` z testem.

- **Poprawki po drugim przeglądzie (świeży subagent):**
  1. Skopiowane światy (kopia albo import ze światami) liczyły się w Statystykach dwa razy: światy o tym samym
     zapisie rekordu (`same_write`) liczą się raz, w pierwszym folderze (najpierw własne launchera). Test.
  2. Ta sama nazwa świata Atum w kilku instancjach blokowała dopasowanie starszego zapisu (reguła 3): teraz
     wystarcza, że pasuje dokładnie jeden z nich.
  3. Import z oficjalnego launchera skopiowałby cały `.minecraft` (assets, libraries, versions, pliki kont):
     import bierze tylko listę dozwolonych (pliki opcji, `config/`, `resourcepacks/`, `saves/` na życzenie).
     Nazwy porównywane bez wielkości liter („Mods”, „Saves” na Windowsie). Test z plikami oficjalnego launchera.
  4. Sprawdzenie „to folder tego launchera” działa na ścieżkach kanonicznych (dowiązania, `..`), jak w imporcie.
  5. W czasie importu formularz był aktywny (można było równolegle utworzyć drugą instancję): przyciski
     zablokowane, Esc anuluje import, błąd przy zamkniętym formularzu idzie do komunikatu.
  Też: panika w zadaniu kopiowania kończy się błędem zamiast wiecznego paska; zmiana listy własnych instancji
  odświeża Statystyki.
- **Płynne paski instalacji** (Start, przycisk Graj, kafelki, własne instancje, Narzędzia): pasek dochodzi do
  postępu wykładniczo (90 ms) zamiast skakać co zdarzenie; przy nowym etapie wraca od razu. Klatki tylko, gdy
  pasek jest w tyle; po instalacji 0 klatek przez 2 s (zmierzone).

- **Ruch na Instancjach:** tło strony (grafika instancji) przechodzi płynnie przy zmianie instancji, jak grafika
  trybu na Starcie (420 ms, potem 0 klatek; zmierzone `SRL_TRACE_RENDERS`). Komunikat pod kartami („Instancja
  usunięta…”) wsuwa się i rozjaśnia przez 220 ms; po nim 0 klatek przez 2 s (zmierzone). Podkreślenie wybranej
  karty rośnie od środka, gdy jej zawartość się pojawia (ten sam zegar 280 ms).

- **Przeniesienie instancji z innego launchera jako własnej:** w formularzu „Nowa instancja”, pod „Na start”,
  instancje znalezione przez import (MultiMC, Prism, MCSR Launcher…) w tej samej wersji gry; wybrana daje nazwę
  domyślną i opcję „Ze światami”. Utwórz kopiuje jej folder jak „Zrób kopię” (bez `mods/`, logów, `.fabric`),
  z paskiem i Anuluj w formularzu; mody będą z legalnego zestawu trybu bazowego. Backend: `import_instance`,
  `CopyFrom::{Instance, Folder}` (jedna ścieżka kopiowania dla kopii i importu), folder danych launchera
  odrzucany. Test na prawdziwym backendzie (bez modów i logów, światy tylko na życzenie, własny folder
  odrzucony); sprawdzone w Xvfb na danych testowych.
- **Oceny po rundzie:** wygląd 8, model instancji 8,5, zawartość 8,5, nawigacja 8,5, teksty 8, odporność 8,5,
  kod 8,5, wartość 9; ruch 7 i wydajność 9 bez zmian.

- **Statystyki z własnych instancji:** światy własnej instancji liczą się jako runy jej trybu bazowego (np.
  „Sub 10 grind” na zasadach RSG to runy RSG). `srl-stats`: `scan_with`/`Stats::local_with` z dodatkowymi
  folderami gry i trybem każdego, `mode_of_world_in`; run bez globalnego rekordu ma id
  `w:custom-<n>/<świat>`, więc nie zderza się ze światem o tej samej nazwie we wspólnej instancji. Backend podaje
  foldery przy starcie, po utworzeniu i po usunięciu instancji; koniec gry własnej instancji odświeża Statystyki
  jak koniec gry trybu. Test na prawdziwych plikach SpeedRunIGT przeniesionych do `custom-1`.

- **Poprawki po przeglądzie (świeży subagent, 7 uwag, wszystkie poprawione):**
  1. „Posprzątaj” mogło na zawsze usunąć świat przemianowany w grze (Edit World zmienia tylko `LevelName`, folder
     zostaje „Random Speedrun #812”) i każdy „New World”. Teraz decyduje sama nazwa świata (folder tylko, gdy
     `level.dat` się nie czyta), a „New World” nie jest już resetem. Test na przemianowany świat.
  2. Podwójne kliknięcie Utwórz robiło dwie instancje o tej samej nazwie: blokada w UI, a backend sprawdza nazwę
     pod tą samą blokadą, pod którą dodaje definicję.
  3. Edycja nazwy albo argumentów JVM mogła przepaść przy szybkim przełączeniu instancji: zapis przed zmianą.
  4. Koniec kopii zamykał otwarty formularz „Nowa instancja” i kasował wpisaną nazwę: kopia ma własne zdarzenie
     `InstanceCopied`, a formularz zostaje.
  5. Dwie paczki o tej samej nazwie usunięte w tej samej sekundzie: druga nadpisywała pierwszą w koszu. Teraz
     dostaje numer. Test.
  6. Po Enter/Esc w znikającym polu skróty nie działały do kliknięcia: fokus wraca do okna.
  7. Argumenty JVM: odrzucane też `-XX:MaxHeapSize`, `-XX:MaxRAMPercentage` i podobne (pamięć) oraz `-Dfabric.*`
     (ładowanie modów spoza legalnego zestawu).
  Sprawdzone w Xvfb: formularz przeżywa koniec kopii, podwójny Enter tworzy jedną instancję, Ctrl+1 działa od razu.

- **Własne instancje na Starcie:** nad kafelkami trybów, po prawej, „Twoje instancje” z linkami do trzech
  pierwszych (dalej „+N”); link otwiera instancję na stronie Instancje. Bez własnych instancji nagłówek bez zmian.

- **Anulowanie kopii instancji:** obok paska „Zrób kopię” jest Anuluj. Kopia zatrzymuje się przed następnym
  plikiem (`CopyPlan::run_until`), skopiowane pliki i rekordy nowej instancji znikają, a strona mówi „Kopiowanie
  anulowane. Nic nie zostało.”. Pasek pokazuje się od razu po kliknięciu („Liczenie plików…”), więc drugie
  kliknięcie nie uruchomi drugiej kopii. Test zatrzymania w `backend/instances.rs`; sprawdzone w Xvfb. Poprawki
  tekstów PL: „Kopiowanie …” zamiast pierwszej osoby, „Klawisze, opcje i paczki z instancji …”.

- **Własne instancje na stronie Instancje** (druga połowa prośby Piotra, backend z worktree scalony wcześniej).
  - Lista po lewej: wbudowane instancje, pod nimi własne („Twoja · gra jak RSG”), przewijana, a na dole „Nowa
    instancja”.
  - Formularz „Stwórz własną”: nazwa (puste pole daje „Moja instancja”, z numerem, gdy zajęta), tryb, na którego
    zasadach gra (Ranked, RSG, FSG, SSG, Draftout; potem stały), i „Klawisze, opcje i paczki z …” z instancji tej
    samej wersji. Zła nazwa daje komunikat pod polem. Po utworzeniu instancja jest wybrana na karcie Mody.
  - Własna instancja ma swój przycisk Pobierz/Graj z postępem (`play_custom`, Anuluj/Zatrzymaj grę), te same karty
    co wbudowane i kartę Mody na swoich zmianach (`set_custom_mod`, własny jar).
  - Karta Ustawienia: nazwa (zapis po Enter albo po wyjściu z pola), pamięć z opcją „Jak w RSG”, argumenty Javy
    sprawdzane przed zapisem („-Xmx8G ustawia pamięć: użyj ustawienia Pamięć wyżej”), „Zrób kopię” (też ze
    wbudowanych, opcjonalnie ze światami, z paskiem postępu) i „Usuń instancję” z pytaniem (folder do kosza).
  - Komunikaty („Instancja usunięta…”) są teraz pod kartami, więc widać je na każdej karcie. Karta Ustawienia
    przewija się. Przyciski Otwórz folder idą przez `open_game_folder`, który nie tworzy folderu usuniętej instancji.
  - Zdjęte wszystkie `#[allow(dead_code)]` z API własnych instancji. Sprawdzone w Xvfb (xdotool): tworzenie, zajęta
    nazwa, kopia z postępem, odrzucony `-Xmx8G`, usunięcie, polski formularz; bezczynność 0 klatek.

- **Skróty klawiszowe** (gdy żadne pole tekstowe nie ma fokusu): Ctrl+1…6 strony z nawigacji, Ctrl+, Ustawienia,
  na Starcie ← i → zmieniają tryb, Enter to Graj; Esc zamyka pytanie na Instancjach i wraca z ekranu gry.
  Sprawdzone xdotool w Xvfb. Na Instancjach: Anuluj przy instalacji, „w użyciu” zamiast opisu w wierszu
  instancji, więcej miejsca pod paskiem postępu.

- **Seed każdego świata** na karcie Światy, z „Kopiuj seed” (po dobrym runie seed jest od razu w schowku).
  `srl-practice::level` czyta teraz też `Data.WorldGenSettings.seed` (1.16+), `Data.RandomSeed` (starsze)
  i `Data.LastPlayed`; lista sortuje po `LastPlayed`, dzisiejsze światy pokazują godzinę. „Zarządzaj instancją”
  obok Graj na Starcie prowadzi do instancji trybu. Strona Instancje czyta folder ponownie, gdy gra się zamknie.

- **Strona Instancje zamiast Trybów** (prośba Piotra: launcher ma myśleć instancjami, z podglądem światów, modów,
  konfiguracji). Nowa zakładka w nawigacji: po lewej instancje (Speedrun 1.16.1 dla Ranked, RSG, FSG i SSG,
  Draftout, Mapy treningowe), po prawej wybrana: Graj z wyborem trybu, liczby (światy, rozmiar, mody, Java) i karty:
  - **Światy:** lista z nazwą z `level.dat`, trybem (dla wspólnej instancji), datą i rozmiarem; Otwórz, Usuń (do
    `<dane>/trash/`). „Posprzątaj N światów z resetów” usuwa na zawsze tylko światy o nazwach z Atum/SeedQueue
    („Random Speedrun #N”, „Set Speedrun #N”, „New World”, „Practice Seed”), po potwierdzeniu; nazwane zostają.
  - **Mody:** dawna strona Mody jako karta (ta sama lista, legalność, własny jar).
  - **Paczki zasobów:** włączanie i wyłączanie (zapis do `options.txt`, atomowo), dodawanie `.zip`, usuwanie do
    kosza; w czasie gry zablokowane, bo gra nadpisuje opcje przy wyjściu.
  - **Konfiguracja, Zrzuty ekranu, Logi:** pliki otwierane w programie systemu, miniatury 24 najnowszych zrzutów,
    koniec `latest.log` na miejscu.
  - **Ustawienia:** pamięć trybu, narzędzia startujące z grą, folder.
  Praca na plikach w tle (`backend/content.rs`, 6 testów). Stare identyfikatory `modes*`/`mods*` otwierają
  Instancje. Strona Tryby usunięta (jej pytanie „Tryby względem Startu” z oceny ekranów tym samym zamknięte).
  Sprawdzone w Xvfb: wszystkie karty, sprzątanie usuwa 44 z 47 światów próbki i zostawia 3 nazwane.
  Własne instancje (tworzenie, kopiowanie, usuwanie) robi równolegle subagent w osobnym worktree.
- **Własne instancje: rdzeń i backend** (prośba Piotra; ekrany robi równolegle wątek nadrzędny). Gracz może tworzyć
  własne instancje, każdą z własnym katalogiem `instances/custom-<n>`. Wspólny katalog 1.16.1, Draftout i trening
  zostają bez zmian.
  - `srl-core`: `CustomId` i `Instance::Custom`; `Instance::id`/`dir_id` zwracają `Cow`, `Instance::from_id`,
    `Paths::game_dir(instance)` (jedno miejsce na katalog każdej instancji), `Paths::trash`,
    `shared::per_mode_file`. Zapis serde starych wartości bez zmian, nowa to `{"custom":n}`.
  - Definicje w `settings.json` (`custom_instances`: nazwa 1–40 znaków, unikalna bez względu na wielkość liter,
    tryb bazowy, pamięć, zmiany modów jak `mod_choices`, argumenty JVM, czas utworzenia). Tryb bazowy daje wersję gry,
    Javę, Fabric, legalny zestaw modów i zasady narzędzi. Argumenty JVM bez `-Xmx`/`-Xms`, `-cp`/`-jar` i agentów
    Javy, wstawiane przed klasą główną.
  - Backend: `play_custom`/`cancel_custom` tą samą ścieżką Graj (własny zapis instalacji, log, plik pid i `mods/`,
    nigdy przełączanie Atum wspólnego katalogu), stan przy starcie z dysku, karta Mody z katalogiem względem trybu
    bazowego. `create_instance` (opcjonalnie z opcjami, `config/` i paczkami zasobów innej instancji),
    `duplicate_instance` (cały katalog bez `mods/`, logów i notatek launchera, światy na życzenie, z postępem, pliki
    nie do odczytu pominięte z notatką), `delete_instance` (odmawia, gdy instaluje, gra albo jest kopiowana; katalog
    trafia do `trash/custom-<n>-<czas>`, nic nie jest kasowane). Listą instancji rządzi backend: `set_settings`
    zmienia tylko istniejące i nie zmienia trybu bazowego. CLI: `--install` i `--launch custom-<n>`, `--status`
    pokazuje własne instancje. `FakeBackend` symuluje to samo (`SRL_FAKE=1`).
  - Sprawdzone: `cargo test --workspace`, `clippy -D warnings` i `fmt` zielone; nowe testy ustawień, kopiowania,
    kosza, rozwiązywania zestawu i pełny cykl na `RealBackend` w katalogu tymczasowym (utwórz, duplikuj, zmień, usuń)
    oraz na `FakeBackend`. Na prawdziwej grze w Xvfb: `--install custom-1` (baza RSG, 22 mody, własny katalog, bez
    `instances/mcsr`), potem `--launch` do ekranu tytułowego w 11 s z `-Dsrl.smoke=yes` w linii poleceń Javy
    i czystym zatrzymaniem.
  - Zostaje: Statystyki nie liczą runów z własnych instancji (srl-stats przypisuje rekordy do katalogów trybów),
    import nie proponuje ich jako celu, ekrany (strona Instancje) są w osobnym wątku. Metody API jeszcze
    niewywoływane przez UI mają `#[allow(dead_code)]` do usunięcia przy podpięciu.

- **Przejście między trybami na Starcie** (prośba Piotra: „brakuje animacji”). Klik w kafelek trybu: nowa grafika
  przenika starą przez 420 ms, a tytuł, opis, Graj, liczby po prawej i pasek pod spodem wjeżdżają 10 px z dołu
  z przezroczystości (280 ms, liczby chwilę później). Klatki tylko w czasie przejścia: w Xvfb 28 klatek na
  przełączenie, potem 0 przez 3 s bezczynności.

- **Wydanie `v0.1.4`** (decyzja Piotra): poprawka wstrzykiwania Toolscreen poniżej. Katalog testowy
  `%LOCALAPPDATA%\srl-test` na Windowsie Piotra usunięty.

- **Toolscreen wstrzykiwany dopiero po otwarciu okna gry** (zgłoszenie Piotra: Ranked padał z
  NullPointerException w Lithium przy kształtach bloków, FSG z naruszeniem dostępu `0xC0000005` złapanym przez
  Toolscreen). Launcher wstrzykiwał DLL-e zaraz po starcie JVM. Wstrzykiwacz z Toolscreen 1.4.7 (bajtkod
  `com.easyinject.Main` i `injector.log` Piotra z MultiMC) czeka na widoczne okno gry z tytułem (co 500 ms, do
  120 s), potem 500 ms, liblogger, 100 ms, Toolscreen.dll. Teraz robimy to samo w tle (`inject_when_ready`),
  więc Ninjabrain Bot, stan „Gra działa” i Stop nie czekają; błąd późnego wstrzyknięcia trafia tylko do logu.
  - Sprawdzone na Windowsie Piotra (osobny `SRL_HOME`, za jego zgodą): wydane 0.1.3 z Toolscreen padło 2 z 2 razy
    (Ranked, FSG), poprawione `srl-tools.exe launch` z Toolscreen doszło do ekranu tytułowego 4 z 4 razy (Ranked 3x,
    FSG), Toolscreen wstrzyknięty, bez wyjątków w jego logu. Bez Toolscreen 0.1.3 też startowało.
  - Budowanie na Windows z WSL: `ring` potrzebuje `clang-cl`; zastępczo clang z `pip install ziglang` w venv
    (`~/.cache/srl-zig-bin/clang-cl` z `/imsvc <ziglang>/lib/include`). Całego `srl-app` tak nie zbudujemy (zasób
    GPUI `gpui.manifest.xml` w `llvm-rc`), więc test szedł przez `srl-tools.exe`.

- **Wydanie `v0.1.3`** (decyzja Piotra): nowe wykrywanie importu poniżej, żeby jego 0.1.2 zaktualizowało się samo.
  Przed tagiem poprawka clippy (`sort_by_key`), na której padł Linux w CI; testy `srl-import` przeszły też jako exe
  na Windowsie Piotra.

- **Import znajduje MultiMC tam, gdzie gracz go trzyma** (zgłoszenie Piotra: jego MultiMC w `C:\Games\MultiMC`
  nie był wykryty, a `Downloads\MultiMC` to junction do niego, którego wyszukiwanie celowo nie śledzi).
  - Dyski: każdy lokalny dysk stały (nie tylko systemowy), a na nim foldery o nazwie launchera w korzeniu i jeden
    poziom niżej (`C:\Games\MultiMC`, `D:\Minecraft\PrismLauncher`), bez folderów systemowych.
  - Zapisy komputera (`srl-import/src/records.rs`): lista uruchomionych programów Windowsa (rejestr HKCU,
    `Compatibility Assistant\Store` i `MuiCache`; foldery `MultiMC.exe`, `prismlauncher.exe`, `polymc.exe`) oraz
    `seenPaths` Jingle (`~/.config/Jingle/options.json`). Tylko tym ścieżkom wolno przejść przez link; pokazujemy
    prawdziwy folder, bez `\\?\`. Z katalogu gry launcher szukany do trzech poziomów wyżej, inaczej sama instancja.
  - Sprawdzone exe z `cargo xwin` na Windowsie Piotra: wszystko razem 0,11 s, same zapisy Windowsa i sam Jingle
    po 0,02 s, za każdym razem wszystkie 4 instancje z `C:\Games\MultiMC`, każda raz. Testy `srl-import` zielone.

- **Wydanie `v0.1.2`** (wybór Piotra). Piotr miał już 0.1.1, najnowsze wydanie, więc launcher słusznie nie miał
  czego pobrać. Ten sam kod, tylko wersja w `launcher/Cargo.toml`, żeby jego instalacja 0.1.1 zaktualizowała się sama.

### 2026-10-31

- **Wydanie `v0.1.1`** do testu samoaktualizacji na Windowsie (wybór Piotra): ten sam kod, tylko wersja
  w `launcher/Cargo.toml`. Test: pobierz `v0.1.0` ze strony wydań, uruchom; launcher ma pobrać 0.1.1 i w Ustawieniach,
  w O programie, zaproponować „Uruchom ponownie”.

- **Bez atrybucji AI w gicie** (prośba Piotra). Cztery pierwsze commity publicznego repo miały linię
  `Co-Authored-By: Claude …`. Opisy przepisane (`git filter-branch --msg-filter`, pliki bez zmian, to samo
  drzewo), `launcher` i `main` wypchnięte z `--force-with-lease`. Wydanie `v0.1.0` usunięte i zbudowane od nowa
  z tagu na nowym commicie, `launcher-latest` przebudowane ręcznym runem, stare runy CI usunięte. Na przyszłość:
  `attribution` wyłączone w ustawieniach Claude Code, reguła w globalnym AGENTS.md i lokalny hook `commit-msg`,
  który wycina takie linie.

- **Samoaktualizacja launchera i numerowane wydania** (subagent, gałąź `launcher`, decyzja Piotra o budowie).
  - **CI:** tag `v<wersja>` buduje i testuje jak push, sprawdza, że tag to `v` plus `[workspace.package]
    version` (inaczej oba joby padają), i publikuje zwykłe wydanie (nie prerelease, oznaczone jako latest), więc
    `releases/latest` GitHuba je zwraca. Obok zipa jest `speedrunning-launcher-windows.zip.sha256` (jedna linia
    w formacie `sha256sum`), także w `launcher-latest`. Podpis: tag podpisuje, gdy są sekret
    `SIGNPATH_API_TOKEN` i zmienna `SIGNPATH_ORGANIZATION_ID`, bez nich buduje bez podpisu; ręczne `sign` jak
    dotąd; push do `launcher` nigdy. `VERSION.txt` ma teraz wersję. actionlint czysty. Tagów ani wydań nie
    tworzyłem.
  - **Nowy crate `srl-update`:** porównanie wersji (semver, bez pre-release, metadane builda się nie liczą),
    wybór assetów, suma z pliku `.sha256`, pobranie zipa przez `Downloader` z srl-core (SHA-256 sprawdzone przed
    użyciem; srl-core ma teraz typowany błąd `HashMismatch`), rozpakowanie do `<dane>/updates/<wersja>/staged`
    ze strażnikiem zip-slip z srl-practice, bez symlinków i powtórzonych nazw, limity 1000 wpisów i 512 MB.
  - **Kiedy podmiana:** od razu po pobraniu i sprawdzeniu, nie dopiero przy „Uruchom ponownie”. Nowy exe jest
    kopiowany obok jako `.new`, działający dostaje nazwę `speedrunning-launcher.old.exe` (Windows pozwala zmienić
    nazwę działającego exe), `.new` bierze jego nazwę, reszta plików zipa jest kopiowana obok; nieudana zmiana
    nazwy przywraca stary exe. Następny start usuwa `.old.exe` (z ponowieniem, bo poprzedni proces może jeszcze
    kończyć). Dzięki temu aktualizacja działa przy każdym następnym starcie, a „Uruchom ponownie” tylko ją
    przyspiesza. Folder bez prawa zapisu (Program Files): nic się nie pobiera, O programie mówi to wprost i daje
    link do wydania.
  - **Aplikacja:** sprawdzenie raz przy starcie w tle (przełącznik „Sprawdzaj aktualizacje automatycznie”,
    domyślnie włączony, pole `auto_update` w `settings.json`) i „Sprawdź teraz” w Ustawieniach, w O programie, ze
    stanami: sprawdzanie, aktualna, pobieranie, gotowa, ręcznie, błąd (offline, limit GitHuba, zła suma, inny).
    Automatyczne sprawdzenie bez sieci tylko pisze do logu. Tylko build z CI na Windowsie (`SRL_COMMIT`); lokalny
    i Linux mówią, że same się nie aktualizują; `SRL_FAKE`/`SRL_DEMO` używają symulacji. „Uruchom ponownie”
    czeka, gdy gra z launchera działa albo coś się instaluje, i mówi dlaczego. Powiadomienie: akcentowy kwadracik
    na zębatce w górnym pasku (jak w VS Code); napis się nie mieścił, bo po polsku pasek przy 1280 px ma wolne
    około 50 px. `SRL_UPDATE_URL` kieruje sprawdzenie na inny serwer (testy). Ekrany `update-*` w README aplikacji.
  - **Testy:** `cargo test --workspace` 338 (było 320): 12 jednostkowych w srl-update (wersje, assety, plik sumy,
    zip-slip, duplikaty, podmiana, wycofanie przy każdym kroku, uprawnienia, sprzątanie), 4 integracyjne z lokalnym
    serwerem HTTP (nowsza wersja pobrana, sprawdzona, rozpakowana i podmieniona; ta sama wersja nic nie pobiera;
    zła suma odrzucona; 404, limit, zły JSON, offline), 2 w aplikacji. Na Linuksie prawdziwy backend
    (`GITHUB_SHA=… cargo build`, `SRL_UPDATE_URL`, Xvfb, `SRL_HOME` w cache): wydanie 0.2.0 pobrane, podmienione,
    stary plik `.old-3` usunięty przy starcie, klik „Restart to update” zamknął launcher i uruchomił nowy plik;
    zła suma: tylko linia w logu; folder tylko do odczytu: komunikat z linkiem; ta sama wersja: nic. Zrzuty
    w `~/.cache/srl-e2e/selfupdate/` (`en/`, `pl/`, `e2e-real-*.jpg`).
  - **Niesprawdzone (tylko Windows):** zmiana nazwy działającego exe, start nowego, usunięcie `.old.exe`,
    Defender przy podmianie, Program Files.

- **Publiczne repozytorium.** Stare repo zmienione na prywatne `speedrunning-launcher-private`. Nowe publiczne
  `mlodygbelmondo/speedrunning-launcher` startuje od jednego commita z drzewem z 7d6a3b1: bez `research/visual/`
  (cudze zrzuty ekranu jako wzory), z `LICENSE` (GPL v3) i `license = "GPL-3.0-or-later"` w `Cargo.toml`. Przed
  publikacją gitleaks przejrzał całą historię i drzewo: tylko fałszywe alarmy (testowy JWT, UUID Notcha, nazwa
  szyfru). Plików Mojanga w drzewie nie ma. Autor commita to adres noreply GitHuba. Lokalny checkout: `origin` to
  nowe repo, `private` stare. Pierwszy push nie uruchomił CI (gałąź `launcher` równa `main`, filtr ścieżek),
  więc run był uruchomiony ręcznie.
- **Przygotowanie podpisu SignPath.** Exe dostaje zasób wersji (`winresource` w `build.rs`: ProductName
  „Speedrunning Launcher”, wersja z crate'a); CI na Windowsie sprawdza go po buildzie. Ręczne uruchomienie
  workflow z `sign` wysyła przetestowany exe do SignPath (`signpath/github-action-submit-signing-request@v3`),
  czeka do godziny na zatwierdzenie, sprawdza podpis (`Get-AuthenticodeSignature`) i pakuje podpisany exe. Bez
  `sign` (każdy push) paczka jest niepodpisana jak dotąd; notatki wydania i `VERSION.txt` mówią, czy podpisana.
  Konfiguracja artefaktu: `launcher/tools/windows/signpath-artifact-configuration.xml`, instrukcja:
  `SIGNPATH.md` obok. README ma „Code signing policy” (role, zasady) i „Privacy” (z jakimi serwisami launcher
  się łączy i kiedy; bez telemetrii).

### 2026-10-29

- **Nowa grafika Ranked: wioska z 1.16.1** (subagent, gałąź `ranked-village` z `launcher` bb8bd65; wybór Piotra
  po ocenie, że słupki z blokami rang wyglądają jak zaślepka). Prawdziwa wioska równinna z seeda 2026: start
  (−816, 32), domy w x −892..−732, z −31..72, staw od zachodu, ciemny las z wielkimi grzybami w tle. Widok
  z (−870, 84, 95) na północny wschód, złota godzina, słońce nisko na zachodzie, chmury w rozmiarze z gry.
  Narzędzia: `mc/find_village.py` (siedem wiosek wokół spawnu, typ i elementy każdej; wybrane spośród trzech
  równinnych), `gen.py` i `extract.py` dla pudełka x −1120..−510, z −280..340, `scene_village.py` (stare
  `scene_ranked.py` zostaje). Jedna poprawka terenu: wylot jaskini przy stawie zasypany do wysokości łąki, bo
  w tej skali był czarną dziurą. Nowe własne tekstury bloków wielkich grzybów (wcześniej udawały je ziemia
  i czerwona wełna), doniczki z roślinami rysują się jako doniczki. Render 1920×1080: 254 s. `hero/ranked.jpg`
  i `card/ranked.jpg` z `make-art.sh` (pozostałe pliki bez zmian), CREDITS i ekran Autorów (EN, PL) wspominają
  wioskę. Zrzuty Startu przed i po, karta i Tryby w `~/.cache/srl-e2e/village/`. `cargo test --workspace`: 320.
  Uwaga: `/tmp` czyści stare pliki, więc Blender i venv w `/tmp/srl/art` były uszkodzone; Blender pobrany na
  nowo z blender.org, venv odtworzony, jar serwera 1.16.1 pobrany z Mojanga do `/tmp/srl/mcworld`.

- **Tymczasowy przełącznik deweloperski** (Piotr): Ustawienia, sekcja „Deweloperskie (tymczasowe)”, „Ukryj taglines
  i podtytuły”. Pole `dev_hide_taglines` w `settings.json`; `copy::is_tagline` wymienia ukrywane klucze (nadtytuły,
  podtytuły stron, opisy trybów na Starcie, wstępy ekranów pełnookienkowych i strony Mody, opisy kart Narzędzi
  i Nauki; treść okien dialogowych zostaje). Zrzuty przed i po w `~/.cache/srl-e2e/taglines/`. Do usunięcia, gdy
  Piotr zdecyduje, które teksty zostają.

- **Mój test po nocnych zmianach** (d463983, Xvfb, pusty katalog danych, `--launch --until-ready`): RSG od zera do
  ekranu tytułowego w 27 s, potem SSG na tej samej `instances/mcsr` w 15 s (doszedł SetSpawn, zeszły WorldPreview
  i LazyStronghold), potem znowu RSG: `mods/` identyczne jak za pierwszym razem. Ninjabrain Bot startował z grą,
  Stop zamknął grę (kod 0) i narzędzia w 0,5 s. CI dla blokady offline i systemu przycisków przeszło (runy
  40803322360, 40806685955). Stare katalogi kompilacji agentów w `~/.cache/srl-target` (ok. 240 GB) usunięte, bo
  dysk był zajęty w 95%.

- **Przegląd kodu przed testem na Windowsie** (subagent, gałąź `review-fixes` z `launcher` cd16051): wspólna
  instancja 1.16.1 i strona Mody (ccf3655), import ustawień (ff80645, f8733a3) i backend M3 (statystyki, konto
  przy Graj, trening), pod kątem utraty danych, Windowsa, współbieżności i złego zestawu modów. Znalezione
  i naprawione, każde z testem, który przed poprawką padał (poza 5.):
  1. Import wkładał ustawienia Atum ze źródła do wspólnej instancji. Z instancji SSG z seedem ten seed stawał się
     ustawieniem RSG (przy świeżej instalacji pierwszy tryb przejmuje to, co leży w folderze), więc RSG resetował
     na ustalony seed. Teraz `srl-import` ma `Target::keeping`, a wspólna instancja nie bierze z importu plików
     Atum (`shared::PER_MODE_FILES`); trening je dostaje. Test `the_shared_instance_keeps_its_own_atum_settings`.
  2. `srl_mods::install`: jar gracza w `mods/` o nazwie jara z zestawu (inny build, bez kopii w magazynie) był
     nadpisywany przy podmianie, zamiast trafić do `mods-removed/`. Na Windowsie przy nazwie różniącej się tylko
     wielkością liter pętla usuwania wyniosłaby świeżo zainstalowany jar (to tylko z rozumowania, Linux tego nie
     pokaże). Teraz taki jar wychodzi przed podmianą, a nazwy porównuję jak system plików. Test
     `a_foreign_jar_under_a_set_jars_name_is_kept_aside`.
  3. „Dodaj własny .jar…” przyjmował Fabric API, który potem szedł do `mods/` każdego trybu, także Ranked (moduł
     `choice` obiecuje, że zakazane mody nigdy nie wchodzą). Teraz jar z `id` zakazanym w każdym trybie jest
     odrzucany z komunikatem. Test `a_players_jar_can_not_be_fabric_api`.
  4. Własny jar o nazwie zajętej przez jar zestawu dostawał nazwę `custom:<hash>-….jar`. Dwukropek na NTFS
     otwiera strumień danych, więc instalacja na Windowsie by padła. Teraz `custom-<hash>-….jar`. Test
     `an_own_jar_with_a_taken_name_gets_a_windows_name`.
  5. Backend przyjmował Graj trybu 1.16.1, gdy inny tryb tej instancji był między instalacją a startem gry
     (chronił tylko przycisk, `sibling_busy`). Wtedy druga instalacja podmieniłaby mody i Atum pod startującą grą.
     Teraz `RealBackend::play` odmawia, jeden Graj na folder gry. Bez testu, bo wymaga prawdziwego backendu.
  Zostaje (niskie, bez poprawki): `shared::switch_to` nie jest odporny na awarię w połowie (padnięcie procesu
  albo błąd I/O między przeniesieniem plików Atum a zapisem `state.json` może skasować zapisane ustawienia Atum
  trybu albo zostawić seed SSG w RSG, okno rzędu milisekund). Migracja na Windowsie: jeśli zmiana nazwy
  `instances\<tryb>` na `mcsr` się nie uda (otwarty plik), Graj tworzy nowy `mcsr`, a stare ustawienia trafiają
  do `old-<tryb>` (nic nie ginie, dotyczy tylko aktualizacji ze starego układu, na Windowsie takich nie ma).
  Niesprawdzone: czy Atum honoruje znacznik `config/mcsr/atum.global` (wspólna konfiguracja SpeedrunAPI w katalogu
  domowym), bo wtedy seed byłby wspólny dla trybów mimo podmiany. Brak blokady jednej kopii launchera (dwa okna to
  dwie osobne blokady instalacji, było tak od M1). Wynik importu nadal mówi „wszystko, co zaznaczyłeś” także
  o wspólnej instancji, choć ustawienia Atum zostają w trybach. Nieprzejrzane: kod ekranów M3 (UI), wnętrze
  `srl-auth`, `srl-practice` (rdzeń miał przegląd), `srl-game`, `srl-tools`. Sprawdzone: `cargo fmt --check`,
  clippy z `-D warnings`, `cargo test --workspace` (320, 9 pominiętych).

- **System przycisków i typografii** (subagent, gałąź `design-system`, jeszcze nie w `launcher`): większa praca
  z oceny ekranów, bez zmiany wyglądu (fiolet, własne rendery, kwadratowe rogi, ząbkowany Graj). Wszystko
  w `v3.rs`, opis w README `srl-app` („Buttons and labels”).
  1. Przyciski: jeden główny na widok (`play_button` albo mały `primary` z tymi samymi ząbkami: „Zaloguj się
     przez Microsoft”, „Gotowe”, „Jak to naprawić”), obrysowany `secondary` (Mody, Importuj…, Otwórz folder,
     Dodaj własny .jar…, Zainstaluj i Graj w wierszu treningu, Zainstaluj i Aktualizuj na karcie narzędzia)
     i tekstowy `tertiary` z cienką linią pod spodem (Anuluj, Wróć, Wyloguj, Resetuj mapę, linki autora).
     Zniknęły białe prostokąty i gołe napisy wersalikami. W wierszu konta stan to cichy „wygasło” przy nazwie,
     a obok jest jeden przycisk „Zaloguj się ponownie” (drugiego już nie było).
  2. Typografia: rozstrzelone wersaliki tylko w górnym menu, na głównych przyciskach i w jednym eyebrow na
     bohaterze (Start, ekran gry, prośba o nick, problem z folderem danych). Tryby, Mody i Nauka mają tę samą
     linię zwykłymi literami (`kicker`). Nagłówki sekcji, etykiety, kolumny tabel, stany („gotowy do gry”,
     „aktywne”, „wygasło”) i segmenty Ustawień są zwykłymi literami w Inter. Plex Mono tylko dla czasów,
     liczb, wersji, rozmiarów i ścieżek (z Plexa wyszły: platformy i autorzy narzędzi, tagi modów, kroki
     logowania, „konto offline” w menu, wiek danych w Statystykach). `upper()` nie robi już „V1.2.2” także
     w „(v1.2.2)” i „„v1.2.2””, z testem. „Active”/„Aktywne” zmienione na „active”/„aktywne” także w kopii
     `srl-auth` (test wymaga zgodności słowo w słowo).
  3. Przewijanie: listy Treningu, Modów i Nauki znikają w tle przy krawędzi, za którą są ukryte wiersze
     (`fade_above`, `fade_below`), więc przewinięty wiersz nie wchodzi już pod nagłówek listy (practice-link).
     Ustawienia mają taki zanik przy dolnej krawędzi okna.
  4. Trening: otwarty wiersz to światy, jeden rząd akcji (Resetuj mapę, Usuń, kreska, Strona autora, Film,
     Ko-fi), jedna linia podziękowań i zwinięte „O mapie” (wymagania, licencja, notka o narzędziach), zamknięte
     domyślnie. Nowy identyfikator ekranu `practice-about` pokazuje je otwarte.
  5. Narzędzia: „Uruchamiaj z grą” to przełączniki w stylu zakładek Modów, z ptaszkiem i fioletowym tłem, gdy
     są włączone; tryb zakazany jest przekreślony. „Draftout go zabrania” jest raz, pod przełącznikami tej karty.
  Sprawdzone: zrzuty przed i po, wszystkie ekrany EN i PL (`~/.cache/srl-e2e/design/{before,after}/{en,pl}`,
  pary w `~/.cache/srl-e2e/design/pairs/`), obejrzane ekrany, które się zmieniły; `cargo fmt --check`, clippy
  z `-D warnings`, `cargo test --workspace` (316, 9 pominiętych). Pominięte, bo to decyzje Piotra: Tryby względem
  Startu, nowa grafika map i renderu Ranked. Lista Treningu nadal jest niska (siatka kategorii zajmuje górę);
  większa lista to zmiana układu, nie systemu. Wskazówka pod Graj („około 500 MB do pobrania”) zostaje w Plexie,
  bo to część przycisku Graj.

- **Blokada grania offline** (sam, w tym wątku): `srl-auth` zapamiętuje w `accounts.json`, że ktoś się tu raz
  zalogował (`signed_in_once`, starsze pliki z kontem na liście liczą się tak samo), a `play_as` w `srl-app` ma nowy
  stan `SignInFirst`: w wersji z client ID, bez aktywnego konta i bez wcześniejszego logowania Graj zatrzymuje się
  z tekstem „Zaloguj się raz kontem Microsoft, żeby grać. Potem zagrasz też bez konta.” i głównym przyciskiem
  „Zaloguj się”. Bez client ID nic się nie zmienia. Testy: polityka w `play_as`, przed logowaniem prośba, po
  logowaniu konto Microsoft, po wylogowaniu offline (na mocku srl-auth), `signed_in_once` przeżywa wylogowanie.
  Poradnik (część D) i README opisują tę zasadę. Nowy identyfikator ekranu `msa-sign-in-first`; zrzuty EN i PL
  w `~/.cache/srl-e2e/offline-gate/`. Etykieta stanu to nadal „błąd”, choć to raczej prośba niż błąd.

### 2026-10-26

- **CI na Windowsie**: krok instalacji sprawdzał jeszcze `instances/rsg/mods`; od jednej instancji 1.16.1 to
  `instances/mcsr/mods` (run 40402357770 padł tylko na tym, kod był w porządku).

- **Szlif UI i tekstów** (gałąź `ui-polish` z `launcher`, subagent, tylko szybkie poprawki z przeglądu):
  - Podpisy: bez podpisów, które powtarzają przycisk (Graj po instalacji, Zatrzymaj grę, Dalej, Importuj, Ponów
    w Narzędziach), bez „z SpeedRunIGT” nad rekordami i źródła nad listą Nauki. Zdania z Plexa przeszły na Inter;
    Plex został dla czasów, wersji, rozmiarów i ścieżek.
  - Start i Tryby: plakietki tylko dla aktualizacji, błędu i działającej gry; bez „NOWE” na warstwach; rozmiar
    pobrania pod Pobierz („około 500 MB”, z pomiaru na czystym katalogu: RSG 537 MB, Draftout 743 MB); tryb 1.16.1,
    którego brat jest zainstalowany, pokazuje Graj („najpierw doda swoje mody”), a nie 500 MB do pobrania.
  - Błędy: brak pamięci daje przycisk „Użyj 3 GB” (ustawia 3072 MB i gra od nowa), Draftout odrzucający mod daje
    „Otwórz mody”, wygasłe logowanie daje „Zaloguj się” bez Ponów, konto dziecka ma „Jak to naprawić” jako główny
    przycisk. Ścieżka logu w tekście zamieniona na przycisk „Otwórz log” (otwiera jego folder).
  - Teksty EN i PL: „my” zamiast „launchera”, zdania bez „X: Y”, bez zapowiedzi („później”, „jeszcze nie w tej
    wersji”), Sign in/Sign out, „…”, kroki logowania mówią, który odmówił. PL: Twierdza, Forteca, Bastion, Okiem
    Kresu, cudzysłowy „…”, twarda spacja po w, i, z, a, o, u (narzędzie `copyedit.py` w `~/.cache/srl-dev/polish/tools`),
    „twój” małą literą. Opisy modów na stronie Mody (`mods.desc.<id>`, z tekstem upstream jako zapasem).
  - Statystyki: bilans sezonu tylko, gdy w sezonie są mecze, „9 runów, żaden ukończony” z liczbą mnogą, „Najlepszy
    czas meczu”. Trening: jedna linia autorów (tylko gdy mówi więcej niż nazwiska), jedna linia Singleplayer, wersje
    nie wielkimi literami. Import: okrągłe radia, bez „Brak w tej instancji” przed wyborem, lista trybów po nazwach.
    Ustawienia: jeden wiersz pamięci dla Ranked · RSG · FSG · SSG i jeden dla Draftout, minimum podane raz.
  - Pominięte: „Narzędzia treningowe w późniejszej wersji” zostaje (to prawdziwy brak), licencje treningu bez
    rozwijania (tylko krótszy tekst), „Wróć do launchera” zostaje.
  - Sprawdzone: `cargo fmt --check`, clippy z `-D warnings`, testy workspace (315 przechodzi, 9 pominiętych), zrzuty
    wszystkich 63 ekranów po angielsku i po polsku w Xvfb.

- **Jedna instancja 1.16.1 i strona Mody** (gałąź `shared-instance`, robione w tym wątku, bez subagentów):
  - `srl-core`: `Instance::dir_id` (`mcsr` dla czterech trybów 1.16.1), moduł `shared`: przeniesienie starych
    folderów (ostatnio grany staje się `instances/mcsr`, pozostałe oddają światy do wspólnych `saves/`, przy
    kolizji nazwy z dopiskiem trybu, i zostają jako `instances/old-<tryb>`; nic nie jest usuwane), ustawienia Atum osobno dla każdego trybu
    (`.srl-modes/<tryb>/`), zapis, który tryb był uruchomiony kiedy.
  - `srl-mods`: blokada gry i instalacji na cały folder (gra RSG blokuje Graj w SSG), notatka zestawu każdego
    trybu w `.srl-mods/`, więc zmiana trybu to nie aktualizacja; moduł `choice`: włącz, wyłącz (razem z modami,
    które go potrzebują), własne jary z magazynu, `concerns` (inna kategoria, spoza listy, własny jar, brak moda
    trybu, Draftout odmawia), `catalogue` dla strony. Fabric API i to, czego nie znosi jar Ranked, nigdy nie
    wchodzi.
  - `srl-stats`: świat ze wspólnych `saves/` liczy się dla trybu, który go stworzył.
  - `srl-app`: strona Mody (zakładki trybów, przełączniki, „Dodaj własny .jar…”, „Przywróć zestaw”), linia pod
    Graj „Nie liczy się w RSG: …”, Graj czeka, gdy inny tryb tej instancji działa; import raz do wspólnej
    instancji.
  - Sprawdzone: `cargo fmt`, clippy z `-D warnings`, testy workspace (wszystkie przechodzą). Na kopii starego
    katalogu testowego (osobne ranked, rsg, fsg, ssg): migracja, potem `--launch rsg` i `--launch ssg` w Xvfb, obie
    gry do ekranu tytułowego; SSG dostało SetSpawn bez WorldPreview i LazyStronghold, seed SSG w Atum wrócił, RSG
    ma pusty. Prawdziwe okno w Xvfb: klik w przełącznik zapisuje wybór, drugi klik go cofa, ostrzeżenie widać na
    stronie Mody i pod Graj. Zrzuty w `~/.cache/srl-e2e/mods/`.
  - Nie sprawdzone: Windows (kopie jarów zamiast twardych linków przy każdej zmianie trybu), prawdziwy jar
    dodany z okna wyboru pliku (test podawał ścieżkę zmienną `SRL_MODS_PICK`), mecz Ranked na wspólnej instancji.

- **Import ustawień, ekran** (`srl-app`, gałąź `import-settings`):
  - Jeden ekran: lista znalezionych instancji (najlepsza zaznaczona), „Wskaż folder…” (folder instancji albo gry),
    cztery pola (ustawienia gry, modów i paczki włączone, światy nie) i jeden przycisk Importuj. Instancja z innej
    wersji niż 1.16.1 daje tylko klawisze, czułość i FOV i ekran to mówi. Na dole jedna linia: Toolscreen i
    Ninjabrain Bot już używają ustawień gracza.
  - Pierwszy start: po wpisaniu nicku ekran importu pojawia się raz; Pomiń albo Importuj zapisuje
    `import_offered` i więcej się nie pokazuje. Potem: Ustawienia, Ogólne, „Importuj…”.
  - Import omija tryb, którego gra właśnie działa albo się instaluje, i mówi o tym. Draftout dalej dostaje
    `onboardAccessibility:false`. Wynik mówi, gdzie są kopie zastąpionych plików.
  - Do testów: `SRL_IMPORT_HOME` (sztuczny katalog domowy do wyszukiwania) i `SRL_IMPORT_PICK` (folder zamiast
    okna wyboru); ekrany `import`, `import-scanning|none|other|importing|done` w `SRL_FAKE`.
  - **Sprawdzenie:** test od zera w Xvfb ze sztucznym MultiMC na Pulpicie i Prismem: pierwszy start, nick, ekran
    importu, Importuj ze światami; wszystkie tryby 1.16.1 mają klawisze, czułość, FOV, paczkę, `config/mcsr`,
    SpeedRunIGT i świat, Draftout tylko 35 ustawień plus `onboardAccessibility:false`, link symboliczny do `~/.ssh`
    pominięty. Drugi start bez oferty, Ustawienia, Importuj…, Wskaż folder, drugi import: nowe ustawienia, a stare
    `options.txt` i `seedqueue.json` w `import-backup-<czas>/`; źródło bez zmian. fmt, clippy i 302 testy zielone.

- **Import ustawień, rdzeń** (`srl-import`, gałąź `import-settings`):
  - Wykrywanie: MultiMC (przenośny, więc szukany w katalogu domowym, na Pulpicie, w Dokumentach, Pobranych do dwóch
    poziomów w dół i w `C:\MultiMC*`; `InstanceDir` z `multimc.cfg`), Prism i PolyMC (`%APPDATA%`,
    `~/.local/share`, Flatpak), MCSR Launcher (z jego kodu: instalator kładzie go w
    `%LOCALAPPDATA%\MCSRLauncher`, instancje są w `launcher\instances\<id>\instance.json`, gra w `.minecraft`),
    oficjalny `.minecraft` i profile z własnym katalogiem gry z `launcher_profiles.json`. Nazwa z `instance.cfg`,
    wersja z `mmc-pack.json`, a bez nich z numeru wersji danych w `options.txt` (2567 to 1.16.1).
  - Ustawienia modów w 1.16.1 (sprawdzone w kodzie modów i w naszych instancjach): mody na SpeedrunAPI trzymają je
    w `config/mcsr/<mod>.json` (StandardSettings, Atum, SeedQueue, Sodium, WorldPreview, SleepBackground), plik
    `<mod>.global` tam oznacza wspólny `~/.config/mcsr/config/`. SpeedRunIGT ma `speedrunigt/options.txt` w katalogu
    gry (albo wspólne, gdy jest `.useglobaloption`). Starsze wersje: `config/standardoptions.txt`,
    `config/atum/atum.properties`, `config/sodium-options.json`. Import kopiuje cały `config/` i te dwa pliki
    SpeedRunIGT.
  - Klawisze wspólne dla 1.16.1 i 26.3: 33 klawisze vanilla, porównane na plikach `options.txt`, które obie wersje
    zapisały w naszych testach. Czułość i FOV mają w obu tę samą skalę.
  - Bezpieczeństwo: źródło jest tylko czytane, linki symboliczne są pomijane, pliki ponad limit (8 MB tekst, 512 MB
    paczka albo plik świata) też. To, co import nadpisuje, trafia najpierw do `import-backup-<czas>/` w tej instancji.
    Istniejące światy zostają.

- **Rdzeń M3 scalony do `launcher`** (workflow `launcher-m3-core`: 3 strumienie, każdy zbudowany, sprawdzony
  przez 2 recenzentów i poprawiony; gałąź `m3-core` 402e0c3).
  - `srl-stats` (m3-stats 73ae9b6): 11 uwag z przeglądu poprawionych. Run z cheatami albo z otwartym LAN-em nie
    zostaje PB; runy z innych launcherów o tej samej nazwie świata nie są doliczane do trybu; wynik w trwającym
    Weekly Race bierze się z tablicy wyścigu.
  - `srl-practice` (m3-practice e692904): 13 uwag poprawionych. Aktualizacja i reset nie kasują starego świata,
    zanim nowy się rozpakuje; Portal Practice nie obiecuje importu, którego wersja 1.3.1 nie ma.
  - `srl-auth` (m3-auth f23ab69): 9 uwag poprawionych. Błąd magazynu haseł przy logowaniu nie generuje po cichu
    nowego klucza (to wylogowałoby inne konta).
  - Konflikty przy scaleniu tylko tam, gdzie obie strony dopisały linie: lista crate'ów, `Cargo.lock` (wersje nowych
    zależności takie same jak w `m3-core`), `THIRD_PARTY_NOTICES.md`, `Paths::tools()` i `Paths::practice_instance()`.
  - **Moje sprawdzenie:** `cargo fmt --check`, `clippy -D warnings` i `cargo test --workspace` zielone po scaleniu.
    Testy na żywo: API Ranked (profil, 20 meczów, Weekly Race #109, 1012 graczy online) i Draftout odpowiadają.
    Wszystkie 13 map zainstalowało się od zera w 13 s z linków autorów, hashe zgodne, każdy świat ma DataVersion
    2567 (1.16.1).

- **Przegląd kodu M2 i poprawki** (wszystkie 17 uwag okazały się prawdziwe, wszystkie poprawione):
  - **5dbe9f1, narzędzia:**
    - waywall w trybie 1.16.1 pobiera teraz `legal-dlls.csv` także na Linuksie. Wcześniej każda gra z waywallem
      ostrzegała, że GLFW nie jest legalne, nawet gdy było. Bez listy notatka mówi, że GLFW nie dało się
      sprawdzić.
    - Konfiguracja waywalla, która trzyma ścieżkę jara Ninjabrain Bota w zmiennej (np.
      arjuncgore/waywall_generic_config), jest rozpoznawana. Wcześniej launcher uruchamiał drugi Ninjabrain obok
      waywalla, a ten z konfiguracji się nie pojawiał.
    - Ninjabrain Bot uruchomiony przez launcher ma plik `logs/tools/ninjabrain-bot.pid`. Gdy launcher się zamknie
      albo padnie w trakcie gry, przy następnym starcie przejmuje Ninjabrain Bota i zamyka go po wyjściu z gry.
      Wcześniej zostawał otwarty na zawsze i uchodził za uruchomionego przez gracza.
    - Dwie gry naraz: wyjście z pierwszej nie zamyka Ninjabrain Bota, którego używa druga.
    - `legal-dlls.csv`, który się nie parsuje, nie psuje instalacji Toolscreen: launcher bierze kopię z dysku.
    - Windows: gdy `LoadLibraryW` w grze trwa dłużej niż 10 s, launcher nie zwalnia pamięci, którą wątek gry może
      jeszcze czytać.
  - **d5a74aa, ekrany:**
    - Autorzy i licencje pokazują tekst dla gracza (`assets/credits.md`, `credits.pl.md`: grafiki, fonty z autorami,
      ikony), a nie notatki deweloperskie. Kolumna Kod ma Pandora Launcher i EasyInjectBundled. Paczka na Windowsa
      ma też licencję ikon Lucide.
    - Nauka: dół listy przygasa, gdy poniżej są ukryte poradniki („Mierzenie oczu” ma 9). Dwie serie otwierają
      playlisty, a MCSR Ranked Explanations ma 8 części, nie 6.
    - Narzędzia: długa ścieżka waywalla jest skracana w środku. Błąd instalacji ma jedną linię szczegółów, więc
      notatka na dole nie dotyka krawędzi okna. Ekran sprawdza narzędzia ponownie przy każdym otwarciu, więc
      waywall zainstalowany przy otwartym launcherze od razu się pokazuje.
    - Statystyki: karty źródeł mają pełną szerokość. Ustawienia: podpowiedź nicku wygląda jak inne podpowiedzi,
      a opis dostępności mówi, że Extra Options działa w trybach 1.16.1.
    - Polski: naturalniejsze zdania i odmiana nazw narzędzi („z Ninjabrain Botem”, „Toolscreena”).
  - **358c6e5:** zrzuty zmienionych ekranów po angielsku i po polsku, Narzędzia z waywallem na długiej ścieżce
    i RSG z Ninjabrain Botem z tego testu (`launcher/shots/m2/e2e-11-rsg-ninjabrain-fix.jpg`).
  - **Sprawdzone** (Linux, Xvfb, wszystko przez `srl-cpu`):
    - `cargo fmt --check`, `clippy -D warnings` (Linux i `x86_64-pc-windows-gnu`), `cargo test --workspace`.
    - RSG od pustego `SRL_HOME` w oknie launchera: instalacja 12,5 s, ekran tytułowy 10,3 s po starcie procesu,
      Ninjabrain Bot otwarty obok gry.
    - Launcher zabity (`kill -9`) w trakcie gry: gra i Ninjabrain Bot działają dalej. Po ponownym starcie log
      mówi „Ninjabrain Bot from an earlier session: Kept”. Wyjście z gry: Ninjabrain Bot zamknięty przez okno
      po 1,6 s, plik pid usunięty. Ponowne Graj działa jak zwykle.
    - `--launch rsg --tools waywall` (waywall na niby w PATH, bez sesji Wayland): `meta/tools/legal-dlls.csv`
      pobrany na Linuksie, gra startuje bez waywalla z notatką.
    - Narzędzia: zaślepka waywall wrzucona do PATH przy otwartym launcherze, potem Statystyki i z powrotem
      Narzędzia: karta zmienia się z „nie zainstalowane” na „zainstalowane”.
    - 0 klatek w spoczynku na learn, learn-eyes, tools, tools-failed, settings, settings-credits i stats. Wszystkie
      60 linków z Nauki odpowiada, playlisty też przez oEmbed.
  - **Moje sprawdzenie 358c6e5** (główny wątek, Xvfb, polski interfejs): `cargo test --workspace` zielone. RSG od
    pustego katalogu w oknie launchera: instalacja 12,0 s, ekran tytułowy 10,3 s po starcie procesu, Ninjabrain Bot
    1.5.2 otwarty obok gry. Zatrzymaj z launchera: gra wychodzi z kodem 0 po 0,5 s, w logu jest `Stopping!`,
    Ninjabrain Bot zamknięty, żadna Java nie zostaje. Ekrany Narzędzia, Nauka i Ustawienia wyglądają dobrze.

- **CI:** run 40319059891 jest zielony (Linux, Windows, publikacja), z commita 4080e70.
  - Windows: instalacja RSG bez okna od pustego katalogu w 9,3 s. Ninjabrain Bot 1.5.2 i Toolscreen 1.4.7 (x64)
    zainstalowane bez okna, trzy pliki sprawdzone `sha256sum`/`sha512sum` z hashami przypiętymi w `srl-tools`.
  - Paczka `speedrunning-launcher-windows.zip` (11 MB) jest w `launcher-latest`. Stara paczka
    `speedrunluncher-windows.zip` jest z tego wydania usunięta.
- **e2a0474:** CI na Windowsie instaluje bez okna Ninjabrain Bota i Toolscreen do pustego katalogu. Każdy plik,
  który launcher zgłosi, CI liczy jeszcze raz (`sha256sum`, `sha512sum`), a hash musi być przypięty w `srl-tools`.
- **12eb960:** zrzuty M2 od nowa, już z nową nazwą: wszystkie nowe ekrany po angielsku i po polsku
  (`launcher/shots/m2/en`, `pl`) i zrzuty z testu E2E (`launcher/shots/m2/e2e-*.jpg`). `idle-pages.sh` szuka
  pliku i okna pod nową nazwą. Każdy nowy ekran w spoczynku rysuje 0 klatek, a 60 linków z Nauki odpowiada.
- **Test od pustego `SRL_HOME`** (Linux, Xvfb, okno launchera, wszystko przez `srl-cpu`):
  - RSG: instalacja 14,15 s razem z Ninjabrain Botem, ekran tytułowy 13,3 s po starcie procesu. Ninjabrain Bot
    1.5.2 otwiera się obok gry, a ekran Gry pokazuje „Narzędzia: Ninjabrain Bot”.
  - Quit Game w grze: launcher zamyka Ninjabrain Bota przez jego okno, gra kończy z kodem 0, RSG jest Gotowe.
  - Ninjabrain Bot wyłączony dla RSG na ekranie Narzędzia (zapisane w `settings.json`): startuje sama gra (0,06 s od
    Graj do procesu), ekran Gry pokazuje „brak”.
  - Zatrzymaj grę z launchera: gra zamknięta w 0,2 s przez jej okno, kod 0, bez zabijania, w logu gry `Stopping!`.
  - Dodatki Draftout włączone, pamięć Draftout 4 GB: instalacja 16,37 s, w `mods/` 8 jarów, w tym
    `NoChatRestrictions-Fabric-MC26.3-v1.3.0.jar` i `no-telemetry-1.10.0.jar`, gra z `-Xmx4096m`. Zatrzymanie:
    0,7 s, przez okno, w logu `Stopping!`.
  - Aktualizacja na ekranie Narzędzia: podłożony Ninjabrain Bot 1.5.1 pokazuje „jest aktualizacja”, Aktualizuj
    pobiera 1.5.2 w 0,8 s z poprawnym sha256 i usuwa 1.5.1.
- **1151e3e:** Narzędzia i Graj na prawdziwym `srl-tools`.
  - Ekran Narzędzia: stan z plików na dysku, prawdziwe instalacje i aktualizacje, przełączniki trybów zapisane
    w `settings.json`. Przekreślony tryb mówi dlaczego (Draftout zabrania Ninjabrain Bota, Toolscreen nie jest
    jeszcze gotowy dla Draftout). Dla waywall ekran mówi, że instaluje się go menedżerem pakietów.
  - Graj: instaluje narzędzia trybu (`Stage::Tools`), startuje grę przez hook narzędzi, uruchamia Ninjabrain Bota
    obok i zamyka go, gdy gra się kończy. Ekran Gry pokazuje, z jakimi narzędziami gra wystartowała, i notatki
    o tych, które nie ruszyły.
  - Mody opcjonalne z Ustawień trafiają do `srl-mods`, pamięć trybu do `srl-game`.
  - CLI: `--tools`, `--install-tool <narzędzie>` (z hashami plików) i `--launch <tryb> --tools <lista|none>`.
    `--launch` wypisuje, jak gra została zatrzymana.
- **edce00b, a5b631b, e7d4856:** połączone strumienie M2:
  - `tools-m2` (af42ba3, a4c50ab, 32bec35): `srl-tools`, hashe sha256 i katalog `tools/` w `srl-core`, zatrzymanie
    gry z zapisem świata w `srl-game`;
  - `screens-a-m2` (01f8d1d, 9f64834, 45ecbec, ee70675): Ustawienia, Narzędzia, przetłumaczone błędy startu gry;
  - `screens-b-m2` (0ea0ab7, 070dc81, 95cda9f): Nauka i Statystyki.
- **bae2d33, 6f27470:** nowa nazwa Speedrunning Launcher i README projektu z nową ikoną.
- **Własny test M1 (poza workflow):** od pustego katalogu, w polskim UI. Wpisanie nicku, potem RSG i Pobierz:
  489 MB w 30 s do menu gry, 33 mody Fabric. Wyjście z gry wraca do „gotowy do gry”, a ponowne Graj uruchamia
  grę w 0,10 s.
- **05fe514, ef3fc34, 7e4df5e, ba37d25, 8750872, 56c04fb:** poprawki po przeglądzie kodu M1.
  - Rozwiązywanie zależności modów zawsze się kończy. Brak zależności drugiego poziomu zapętlał je bez końca.
  - Tryb, który traci własny mod (np. FSG bez FSG Mod), daje błąd zamiast instalować się bez niego. Ranked zostaje
    przy swoim jarze, bo ma Lithium w środku.
  - Jary z `mods/`, których launcher nie ma u siebie, nie są kasowane. Trafiają do
    `instances/<tryb>/mods-removed/<data>/`. Na Windowsie jary są kopiowane, nie linkowane.
  - Listy (Mojang, Fabric, mcsr-meta): gdy jest kopia na dysku, jedna próba z limitem 4 s. Przy sieci, która nie
    odpowiada, instalacja RSG trwa 8,1 s, a w przeglądzie trwała 97 s. Zła odpowiedź też wraca do kopii.
  - Fabric: gdy wybranej wersji loadera nie da się pobrać, launcher bierze najnowszą zainstalowaną i ostrzega.
  - Natywne biblioteki są sprawdzane plik po pliku, a brakujące wracają przy instalacji.
  - Każda wersja Javy ma osobny katalog. Instalator Javy odrzuca pliki pod linkiem.
  - Błąd Graj mówi, co się stało (brak sieci, instalacja albo start gry), i podaje ścieżkę logu z ostatnimi
    liniami gry.
  - Osobny ekran, gdy katalogu danych nie da się zapisać.
  - Nick w Ustawieniach zapisuje się, gdy edycja się kończy: Enter, kliknięcie obok albo wyjście z Ustawień.
    Pusty albo błędny nick wraca do zapisanego.
  - Pamięć ma minimum dla trybu. Ekran Gry pokazuje nick i pamięć tej gry, nie bieżące ustawienia.
  - Polska odmiana liczebników, stan „czekam”, gdy instaluje się inny tryb, i opisy trybów bez obietnic, których
    launcher nie spełnia.
- **Test po poprawkach** (Linux, Xvfb, okno launchera):
  - RSG od pustego `SRL_HOME`: proces gry 14,4 s po kliknięciu, ekran tytułowy 19,6 s po starcie procesu, Fabric
    ładuje 33 mody. Wyjście z gry wraca do Gotowe.
  - Bez sieci: SSG pokazuje brak internetu i Ponów. RSG z dorzuconym jarem dla 26.3 pokazuje powód od Fabrica,
    po polsku też.
  - Bez okna: obcy jar przeniesiony do `mods-removed` z tym samym sha512; usunięty `liblwjgl.so` wrócił; Fabric
    0.20.2 z listy bez sieci daje 0.20.1 z ostrzeżeniem; Ranked ma 23 jary z MCSR Ranked 6.0.4 i Lithium, FSG ma
    23 jary z FSG Mod i Atum.
  - `cargo test`, fmt i clippy (Linux i cel Windows) przechodzą.
- **CI:** run 40288172742 jest zielony (Linux, Windows, publikacja). Na Windowsie instalacja RSG bez okna od
  pustego katalogu: 7,8 s, 22 mody, 518 MB, Java w `java\java-runtime-delta-21.0.7`. Nowa paczka
  `speedrunluncher-windows.zip` (10 MB) jest w `launcher-latest`.
- **93ea21c, 2a78c86, c961d8b:** połączone strumienie `srl-game`, `srl-mods` i `srl-app`.
- **88f52d6, 0b16b2b, dfdf2fe:** poprawki na styku crate'ów:
  - postęp mówi, czy liczy bajty, czy pliki;
  - znaczniki ekranu tytułowego w logu gry są wspólne dla launchera i testowego CLI;
  - zestaw Draftout jest przycięty do listy modu Draftout.
- **bd2ef24:** prawdziwy backend i tryb bez okna.
  - Graj: listy modów, instalacja gry, instalacja modów, start na koncie offline, potem Gra i powrót do Gotowe.
  - Gra, która padnie przy starcie, daje błąd z kodem wyjścia i ścieżką logu. Gdy Fabric zatrzyma start, launcher
    zamyka grę i pokazuje powód.
  - Ustawienia (nick, język, pamięć, ostatni tryb) są w `settings.json`.
- **240ab86, 93eaf5d, 38ba6d6, a1c20ee, 492650f:** CI na Linuksie i Windowsie z paczką `launcher-latest`, opis
  architektury w `launcher/README.md` i zrzuty `launcher/shots/m1/e2e-*.jpg`.
- **Test od pustego `SRL_HOME`** (Linux, Xvfb, okno launchera):
  - RSG: od kliknięcia Pobierz do procesu gry 9,8 s (pobieranie 489 MB). Ekran tytułowy 12,9 s później. Fabric
    ładuje 33 mody, w tym 22 jary z zestawu. Wyjście z gry wraca do Gotowe.
  - Ponowne Graj, wszystko już pobrane: 130–131 ms od kliknięcia do procesu gry (trzy pomiary), okno gry po 5,4–5,8 s.
  - Ranked: 48 modów (23 jary, mcsrranked 6.0.4). Gra staje na ekranie zgody MCSR Ranked.
  - FSG: 34 mody, ekran tytułowy. SSG: 33 mody, ekran tytułowy.
  - Draftout (26.3, Java 25): 67 modów (6 jarów), ekran tytułowy.
  - Wcześniejszy przebieg z dopisaną Planifolią w Draftout: błąd z powodem od Fabrica po około 2,5 s, a Ponów
    działa. Bez sieci tryby są Gotowe od razu i gra startuje.
- **CI:** run 40278499668 jest zielony.
  - Linux: fmt, clippy, testy i build release.
  - Windows: fmt, testy, build release i instalacja RSG bez okna od pustego katalogu: 8,4 s, 22 mody, 518 MB.
  - Paczka `speedrunluncher-windows.zip` (10 MB) jest w prerelease `launcher-latest`.
  - Pierwszy run padł na dwóch testach `srl-game`, które na Windowsie budowały linuksowe ścieżki z `\`. Poprawione
    w a1c20ee.
- **Gałąź `launcher`:** szkielet prawdziwej aplikacji (`launcher/`) i start workflow dla M1.
- **747c3a5:** poprawka migania tła w GPUI. Miganie miało dwie przyczyny:
  - przejścia między stronami rozjaśniały tło z 10 do 18,5 jasności;
  - przy pierwszym wyborze trybu przez 1–2 klatki był widoczny pusty kadr.

  Sprawdzone na nagraniach 60 fps. Zrzuty ekranu są identyczne co do piksela, a zużycie pamięci się nie zmieniło.
- **36c2a9d, 24ce6a8:** dwa przebiegi na Windowsie.
  - GPUI: około 320 ms ciepłego startu i 16–19 % CPU przy przewijaniu.
  - Tauri: 385 ms i 45–50 %.
  - Electron: około 1 s i 51–54 %.
  - Pomiar wyświetlonych klatek przez PresentMon nie działa na trzech monitorach.
- **a812897:** nowe rendery bastionu, RSG (sama wyspa) i SSG (wrak na plaży).

### 2026-10-25

- **b560e29:** scene v3, czyli shell, Start, Tryby, Trening i własne rendery.
- **8944a04:** polska wersja z przełącznikiem.
- **0896734:** klikalny prototyp wszystkich ekranów.

### 2026-10-24

- **04cf643, 90ebcae:** raporty z badań, bake-off i pierwsze wyniki z Linuksa.
