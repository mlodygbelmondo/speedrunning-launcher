# Logowanie Microsoft: rejestracja aplikacji i zgoda Mojanga

Instrukcja dla Piotra. Logowanie Microsoft w launcherze jest gotowe (crate `srl-auth`), ale wyłączone: potrzebuje
**naszego własnego identyfikatora aplikacji (client ID)**, który Mojang musi dopisać do swojej listy dozwolonych
aplikacji. Bez tego Minecraft odrzuca logowanie, a MCSR Ranked i Draftout nie działają (mod Ranked pyta
`api.minecraftservices.com/player/certificates` tokenem z gry).

Cudzego client ID (Prism, MultiMC, Pandora) użyć nie wolno: warunki platformy tożsamości Microsoftu zabraniają
dzielenia się poświadczeniami, a Mojang zatwierdza każdą aplikację osobno. Launcher sam odrzuca znane cudze ID.

Źródło wszystkiego poniżej: `research/03-auth-and-launcher-core.md`, punkt 1.4. Przyciski i nazwy w portalu Azure
podaję po angielsku, bo tak je zobaczysz; Microsoft czasem je przestawia, więc szukaj po nazwie.

## Czego potrzebujesz

- Konta Microsoft dla projektu (najlepiej osobnego, nie prywatnego), żeby rejestrację dało się kiedyś przekazać.
  Ten sam adres e-mail podasz w Azure i w formularzu Mojanga: Mojang je porównuje.
- Publicznej strony projektu (wystarczy GitHub Pages), publicznego repozytorium i krótkiej notki o prywatności.
- Około pół godziny na część A i B. Na odpowiedź Mojanga czeka się od 2 do 8 tygodni.

## A. Katalog (tenant) w Azure

Rejestracja aplikacji poza katalogiem jest dla zwykłych kont Microsoft wycofywana, więc najpierw katalog:

1. Wejdź na <https://azure.microsoft.com/free> i zarejestruj się kontem projektu.
   - Powstanie katalog „Default Directory”. Rejestracje aplikacji są w nim bezpłatne.
   - Azure może poprosić o kartę płatniczą do weryfikacji tożsamości. Tego nie sprawdzałem; sama rejestracja
     aplikacji nic nie kosztuje.
2. Zaloguj się na <https://entra.microsoft.com> tym samym kontem.

## B. Rejestracja aplikacji w Microsoft Entra

1. **Identity → Applications → App registrations → New registration.**
2. **Name:** `Speedrunning Launcher`.
   - Nazwa nie może zawierać słów: Mojang, Minecraft, Microsoft, Live, Xbox, Discord, Hypixel. Inaczej Mojang odrzuci
     wniosek.
3. **Supported account types:** „Personal Microsoft accounts only” (tylko osobiste konta Microsoft).
   - `XboxLive.signin` działa tylko dla kont osobistych (adres `consumers`), a launcher zawsze z niego korzysta.
4. **Redirect URI:**
   - platforma: „Public client/native (mobile & desktop)”;
   - adres: `http://localhost` (dokładnie tak: bez portu, bez ukośnika na końcu, bez `https`).
   - Launcher za każdym razem otwiera inny, wolny port (`http://localhost:54321` itd.). Microsoft przy adresach
     `localhost` ignoruje port, więc jeden wpis wystarczy. Nie dodawaj kilku wpisów różniących się tylko portem.
   - Adresu `http://127.0.0.1` nie dodawaj: portal go nie przyjmuje, a launcher go nie używa.
5. Kliknij **Register**. Na stronie **Overview** zapisz:
   - **Application (client) ID**: to jest nasz client ID (GUID, np. `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`);
   - **Directory (tenant) ID**: potrzebny do formularza Mojanga.
6. **Authentication → Advanced settings → Allow public client flows → Yes**, potem **Save**.
   - Bez tego nie działa logowanie kodem (telefon, QR): Microsoft zwraca błąd AADSTS70002 lub AADSTS7000218,
     a launcher pokazuje „rejestracja aplikacji jest źle ustawiona”.
   - Kod urządzenia nie potrzebuje osobnego adresu przekierowania.
7. **Nie twórz** sekretu ani certyfikatu (**Certificates & secrets** zostaje puste). Launcher jest aplikacją
   publiczną: zabezpiecza go PKCE, nie sekret, a sekret w programie i tak dałoby się wyciągnąć.
8. **API permissions:** niczego nie dodawaj. Launcher prosi o `XboxLive.signin offline_access` w chwili logowania.
   Domyślne `User.Read` może zostać.
9. Opcjonalnie **Branding & properties**: logo, strona domowa, link do notki o prywatności. Weryfikacja wydawcy
   (Publisher verification) wymaga konta w Microsoft Partner Network i do zgody Mojanga nie jest potrzebna. Bez niej
   ekran zgody pokazuje „niezweryfikowany”.

Client ID nie jest tajny (widać go w każdym linku logowania), ale w kodzie go nie zapisujemy: launcher czyta go ze
zmiennej `SRL_MSA_CLIENT_ID`, przy budowaniu albo przy uruchomieniu.

## C. Sprawdzenie przed wysłaniem wniosku

Część łańcucha działa bez zgody Mojanga, więc da się sprawdzić, czy rejestracja jest dobra.

Uruchom test **dwa razy**, bo każda droga sprawdza inny punkt rejestracji: przeglądarka adres przekierowania
(B.4), kod z telefonu ustawienie „public client flows” (B.6). Jedna udana droga nie mówi nic o drugiej.

**Test 1, przeglądarka** (sprawdza B.4):

```sh
cd launcher
SRL_MSA_CLIENT_ID=<client ID> SRL_HOME=$HOME/.cache/srl-dev/msa-test \
  cargo run -p srl-auth --example msa-login
```

- Otworzy się przeglądarka ze stroną logowania Microsoft. W terminalu pojawi się też kod do telefonu: **nie wpisuj
  go**, zaloguj się tylko w przeglądarce.
- Musisz zobaczyć w terminalu linię `... SignedIn(Browser)`. Dopiero ona znaczy, że droga przez przeglądarkę działa.
  Jeśli zamiast niej widzisz `SignedIn(DeviceCode)`, test 1 się nie liczy: powtórz go bez wpisywania kodu.
- Błędy rejestracji przy tej drodze **nie trafiają do terminala**. Microsoft pokazuje je na swojej stronie w
  przeglądarce, a terminal tylko czeka (do 10 minut) i kończy się przekroczeniem czasu. Sprawdzone na żywo 26.10.2026:
  nieznany client ID daje stronę login.live.com z tekstem `unauthorized_client: ...`, bez powrotu do launchera. Patrz
  więc na stronę w przeglądarce:
  - `AADSTS50011` albo „invalid redirect_uri” / „redirect URI ... does not match”: punkt B.4 (adres
    `http://localhost`, platforma „Public client/native”);
  - `unauthorized_client` / „does not exist or is not enabled for consumers”: zły client ID albo punkt B.3
    (konta osobiste).

**Test 2, kod z telefonu** (sprawdza B.6): to samo polecenie z `-- --no-browser` na końcu
(`cargo run -p srl-auth --example msa-login -- --no-browser`). Przeglądarka się nie otworzy (link w terminalu
zignoruj); wpisz kod z terminala na <https://www.microsoft.com/link> (albo na telefonie). Musi pojawić się linia
`... SignedIn(DeviceCode)`.

Wynik obu testów:

- Przed zgodą Mojanga poprawny koniec to: **„Mojang hasn't approved this launcher for Microsoft login yet”**
  (`auth.error.client_not_approved`). Znaczy to, że Microsoft i Xbox Live przyjęły logowanie, a Minecraft odpowiedział
  403 „Invalid app registration”.
- Po zgodzie Mojanga ten sam test kończy się linią `Logged in: <nick> (<uuid>)`. Tak poznasz, że zgoda przyszła, nawet
  jeśli Mojang nie odpisze.
- Błędy, które pokaże terminal (głównie w teście 2):
  - „the client ID is unknown to Microsoft”: literówka w ID albo zła rejestracja;
  - „public client flows are not allowed”: punkt B.6.

## D. Formularz Mojanga (AppID Review)

**Konta offline są już rozstrzygnięte** (Piotr, 2026-10-26, tak jak w Prism):

- Formularz mówi wprost, że aplikacje omijające uwierzytelnianie, sprawdzanie posiadania gry lub licencji nie będą
  zatwierdzone. We wrześniu 2026 Mojang odrzucił launcher UN_Nexo, który oferował granie offline. Autorzy usunęli tę
  funkcję i wysłali wniosek ponownie.
- Dlatego wersja z client ID pozwala grać offline dopiero wtedy, gdy ktoś raz zalogował się na tym komputerze kontem
  Microsoft, które ma grę. Wcześniej Graj prosi o zalogowanie. Wylogowanie potem nie zamyka grania offline.
- Wersja bez client ID (każda, dopóki Mojang nie zatwierdzi wniosku) gra offline jak dotąd, więc testy w czasie
  weryfikacji działają bez konta.
- Kod: `play_as` w `launcher/crates/srl-app/src/backend/account.rs`, jedyne miejsce, które o tym decyduje.

Wypełnianie:

1. Otwórz <https://aka.ms/mce-reviewappid> (formularz „AppID Review”). Zasady są na <https://aka.ms/AppRegInfo>.
2. Pola:
   1. Potwierdzenie, że znasz EULA i Usage Guidelines (<http://aka.ms/mcusageguidelines>). Przeczytaj je naprawdę.
   2. **Contact email:** ten sam adres co w Azure.
   3. **Request type:** „New AppID for Approval”.
   4. **Application name:** `Speedrunning Launcher`.
   5. **Application (client) ID:** z punktu B.5. Jeden identyfikator na jeden wniosek.
   6. **Directory (tenant) ID:** z punktu B.5.
   7. **Website or domain:** strona projektu (np. GitHub Pages) albo repozytorium.
   8. **Justification:** tekst poniżej. Wniosków bez dobrego uzasadnienia Mojang nie rozpatruje.
   9. **Other information:** link do notki o prywatności i do kodu logowania (`launcher/crates/srl-auth`).
3. Wyślij **raz**. Mojang przegląda wnioski raz w tygodniu; kolejne zgłoszenia nie przyspieszają sprawy. Jeśli po
   4–6 tygodniach nic się nie zmieni (test z części C dalej kończy się 403), napisz na `enforce@minecraft.net`
   z client ID.

### Projekt uzasadnienia (do wklejenia po angielsku)

Popraw nawiasy. Zdanie o graniu offline pasuje do launchera (patrz wyżej), więc zostaw je bez nawiasów.

```text
Speedrunning Launcher is an open-source desktop launcher for Minecraft: Java Edition speedrunners
(source: <repository URL>, website: <site URL>). It installs the official game and the mods that the
speedrunning community allows, and starts the game for players of MCSR Ranked and other speedrun
categories.

It uses the Microsoft identity platform as a public client (authorization code with PKCE through the
system browser, and the device code flow) to sign users in with their own Microsoft account. It then
calls Xbox Live and the Minecraft services API (login_with_xbox, entitlements and /minecraft/profile)
to verify that the account owns Minecraft: Java Edition, and launches only the official game with the
user's own access token. MCSR Ranked requires a valid token, as its mod requests the player's
certificates from api.minecraftservices.com.

Tokens stay on the user's device, encrypted with a key kept in the operating system's credential
store, and are sent only to Microsoft, Xbox Live and Mojang. The launcher has no cracked or no-auth
mode, does not bypass ownership or license checks, and [offline play is only offered to users who
have signed in with an account that owns the game].
```

Po polsku, żebyś wiedział, co podpisujesz: launcher jest otwartoźródłowy, loguje własnym kontem Microsoft gracza
(przeglądarka z PKCE albo kod), sprawdza przez Xbox Live i API Minecrafta, czy konto ma grę, i uruchamia tylko
oryginalną grę z tokenem gracza. Tokeny zostają na komputerze, zaszyfrowane kluczem z systemowego magazynu haseł,
i trafiają tylko do Microsoftu, Xbox Live i Mojanga. Nie ma trybu pirackiego ani obchodzenia sprawdzania licencji.

### Notka o prywatności (minimum)

Zdania, które są prawdziwe dla `srl-auth` i które możesz opublikować na stronie:

- Launcher nie widzi hasła: logujesz się na stronie Microsoftu.
- Tokeny logowania są zapisane tylko na Twoim komputerze, zaszyfrowane kluczem trzymanym w systemowym magazynie haseł
  (Menedżer poświadczeń Windows, Pęk kluczy macOS, Secret Service na Linuksie). Bez magazynu haseł klucz leży
  w pliku dostępnym tylko dla Twojego konta.
- Launcher wysyła tokeny tylko do Microsoftu, Xbox Live i Mojanga. Gra dostaje token na czas sesji, jak w oficjalnym
  launcherze; mody (np. MCSR Ranked) używają go według własnych zasad.
- Wylogowanie usuwa tokeny z komputera.

## E. Po zgodzie Mojanga

1. Sprawdź to testem z części C: powinna pojawić się linia `Logged in: ...`.
2. Wpisz client ID do budowania:
   - w GitHub: **Settings → Secrets and variables → Actions → Variables**, zmienna `SRL_MSA_CLIENT_ID`;
   - w `.github/workflows/launcher.yml` przekaż ją do kroku budowania (`env: SRL_MSA_CLIENT_ID: ${{ vars.SRL_MSA_CLIENT_ID }}`).
     `option_env!` wbudowuje ją wtedy w program.
3. Dopiero wtedy ekran konta w launcherze może pokazać przycisk „Zaloguj przez Microsoft”. Bez client ID pokazuje,
   że logowanie Microsoft nie jest jeszcze dostępne w tej wersji.
4. Nie zmieniaj później client ID bez potrzeby: każde konto zalogowane starym ID musi zalogować się ponownie.
