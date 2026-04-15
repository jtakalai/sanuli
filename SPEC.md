# määrittely 1.0: monuli

uusi moodi sanuliin: neluli mutta N:lle sanalle, missä N voi olla vaikka 100.

yrityksiä on N+1, ja eka on taatusti väärin, eli jos eka olisi oikein, se sana vaihdetaan. Sen jälkeen jokaisen on onnistuttava.

nelulin UI on vain kopioituja sanulilautoja pienempinä; monuliin semmoinen ei toimi.

monulin UI: jokainen sana näkyy erikseen väritettynä: vihreät, niiden väleissä sellaiset keltaiset, joita ei ole viherretty (tässä pitää ottaa huomioon jos kirjaimen monikertaa on kokeiltu)
- parasta aloittaa yhden sanan esittämisestä
- testit jne. kuntoon ennen kuin alkaa tunkata uutta moodia sanuliin
- kokonaan uusi tapa esittää sanan arvaukset tiiviisti, kannattaa designata huolella!

varmaankin kaksivaiheinen UI:
- "monulinäkymä" missä voi scrollata (ja ehkä zoomata) ja nähdä kaikki tarjolla olevat ratkaisemattomat sanat, ehkä jopa järjestettynä
- yksittäisen sanan voi valita, jolloin aukeaa tavallinen "sanulinäkymä", joka näyttää sen nimenomaisen sanan värityksen aiemmilla arvauksilla (joita siis voi olla todella paljon!)

### Monulinäkymä

Näppäimistöä ei väritetä osumien mukaan
- koska sanoja on todella paljon, ei näppäimistön värittämisestä osumien mukaan ole mitään hyötyä
- näppäimistön voisi värittää käytettyjen kirjainten mukaan: esim. vaaleansinisellä ne jotka on jo käytetty
- tietysti jos arvattua kirjainta ei todella esiinny (enää) missään (ratkaisemattomassa) sanassa, voisi väritys olla musta samoin kuin sanulinäkymässä

Monulinäkymässä sanat näkyvät ovat skrollattavana listana
- toimii hyvin korkealla/portrait ruudulla, entäs leveällä/landscape?
- sanaa klikkaamalla aukeaa sanulinäkymä kyseiselle valitulle sanalle

Kukin sana on vain yksi rivi, joka koostaa aiempien arvausten osumat
- väritysstrategia:
	- missä tahansa arvauksessa vihreäksi saatu kirjain on vihreänä paikallaan
	- yli jäävissä ruuduissa keltaisella näytetään ne jollain arvauksella keltaisiksi saadut kirjaimet, jotka eivät ole vihreällä
	- jos sama kirjain on jossain arvauksessa ollut monta kertaa, silloin se sama kirjain käsitellään useampana kirjaimena: yksi voi olla vihreä ja toinen keltainen
	- loput ruudut ovat mustia ilman kirjainta
- voiko niin käydä ettei keltaiset mahdu?
	- keltaisia ei näytetä, jos on jo vihreä tarjolla; vaikka tietysti ei tiedetä, onko sanassa toinen sama kirjain, jolloin keltainen olisi ok näyttää
	- tällä tavoin sanan esitys mahtuu 5 (tai 6) laatikkoon, jos mustia ei näytetä; vihreitä plus keltaisia ei voi olla enempää kuin sanassa on kirjaimia koska keltaisia on enintään sanan kirjainten verran, ja jokainen vihreä syö pois joko yhden keltaisen tai yhden mustan ruudun
	- EI VOI käydä siis
	- ja muutenkin, keltaisia voi työntää samaan ruutuun enempi, ks. tarkennukset alla
- entä tilanne, jossa 4 vihreää on löytynyt, ja jäljelle jääväkin kirjain on löytynyt mutta keltainen
	- silloin se on piirrettävä keltaisella ainoalle jäljelle jäävälle paikalle, vaikkei siinä enää ole arvattavaa
	- EI HAITTAA, ja saattaahan sana tulla valmiiksi muutenkin vahingossa, kun keltaiset piirretään vahingossa paikoilleen
- entä tilanne, jossa 5 vihreää on löytynyt, mutta ne eivät ole samassa arvauksessa, ts. sanaa ei ole arvattu mutta se näkyisi monulinäkymässä kokonaan vihreänä
	- EI HAITTAA sinänsä, mutta selvästikin kokonaan arvatut sanat pitää ehkä esittää niin että erottuu tuo tilanne?
- kokonaan arvatut sanat siirretään monulinäkymän hännille siinä järjestyksessä kuin ne arvattiin
	- tällöin viiden vihreän sanat, joita kuitenkaan ei ole arvattu, näkyvät monulilistan kärjessä, kun taas arvatut sanat ovat viimeisen ei-täysin-vihreän sanan jälkeen
- lisäratkaisu: rendataan erotin (esim. täysin musta rivi) ennen ratkaistujen osiota
	- monulilistasta tulee tällöin N+erotin pituinen, ei haittaa
	- erottimessa lukee "Ratkaistut sanat:"

Jos sanoja on enemmän kuin ruudulle mahtuu luettavana, näytetään ensin "kokonaisnäkymä" eli sanat puristetaan vielä pienemmiksi ja kirjaimia ei piirretä vaan pelkät värit.
- puristetut sanat menevät monelle palstalle
	- koska sanoja on tunnettu määrä, jos palstoja mahtuu sivusuunnassa esim 4, niin sitten jokainen palsta sisältää N/4 sanaa "tornina"
- 2-step zoom: kokonaisnäkymässä ensin klikataan palstaa, jolloin aukeaa "monulinäkymä" klikatusta palstasta klikatun kohdan ympäriltä
- jos sanat eivät puristettunakaan mahdu sivulle, sitten sivua skrollataan

Jos sanoja on tarpeeksi vähän (max 10? Riippuu ruudun koosta?), ei tarvita zoomia vaan kaikki sanat ovat suoraan yhdessä monulinäkymässä, jota voi skrollata.

### Sanulinäkymä

Näppäimistö väritetty valitun sanan ja aiempien arvausten mukaiseksi

Paluunappi monulinäkymään

### Testitapauksia monulinäkymän sanaesitykseen
- vihreät .A..I  keltaiset  A.L..
	- tärkeä erotus: montako kertaa A:ta on kokeiltu samassa sanassa?
		- jos vain kerran missään sanassa, keltaista A ei näytetä
		- jos kahdesti jossain sanassa, keltainen A näytetään
			- esim. jos aiemmin arvattu PASTA
	- jos sana on KAALI, keltainen A on oikeinkin näyttää
	- jos sana on LAHTI, keltainen A olisi väärin näyttää
		- tällöin tietysti olisi aiemmin nähty, että A:ta on vain yksi
			- esim. PASTA antaisi yhden vihreän ja yhden mustan A:n

# tarkennus 1.1: monta keltaista samassa ruudussa

Monulinäkymässä pitäisi näkyä, missä kohdissa vääriin kohtiin arvatut kirjaimet ovat. Samaan kohtaan voi tulla useita vääriä arvauksia.

Silloin pitäisi samassa ruudussa olla useampi keltainen kirjain, kuitenkin niitä voi olla maksimissaan neljä viiden kirjaimen sanassa (viimeistään viides olisi oikein). Siispä jos samassa kohdassa on useampi keltainen kirjain, ne pitää merkitä pienemmällä. Yksi mahdollisuus olisi tehdä niistä neljänneksen kokoisia (puolikas korkeus ja leveys).

Kukin monulinäkymän arvausrivi on siis viisi (tai kuusi) kappaletta jotain seuraavista:
- tyhjä ruutu
- vihreätaustainen kirjain
- keltataustainen kirjain
- 2x2 ruudukko, jossa:
    - keltataustainen kirjain
    - tyhjä ruutu

**Esimerkki 1.1.1**: oikea sana LAHTI
    - arvaus 1: KAALI (L väärässä paikassa 4)
    - arvaus 2: TARHA (H väärässä paikassa 4)
	- haluttu väritys olisi:
		- keltataustainen T
		- vihreätaustainen A
		- tyhjä ruutu
		- 2x2 ruudukko, jossa:
			- keltataustainen L
			- keltataustainen H
		- vihreätaustainen I

# tarkennus 1.2: keltainen, joka on jäänyt vihreän alle

Joskus keltainen väärä arvaus jää myöhemmän vihreän arvauksen alle (tai ehkä aiemmankin). Jos sama kirjain on päätynyt keltaiseksi myös muualle, se näytetään mieluummin siellä. Jos taas kirjaimelle ei ole keltaista arvausta ruudussa, jossa ei ole vihreää, se pitää näyttää jotenkin muuten.

Jotta vältetään se, että keltainen olisi hämäävästi väärässä ruudussa, tarvitaan ylimääräinen ruutu keltaisille, jotka ovat jääneet ilman ruutua. Lisäruutu sijoitetaan sanan perään, väliin puolikkaan ruudun levyinen erotin. Jos useampi keltainen on jäänyt ilman ruutua, lisäruutu jaetaan samalla tavalla kuin kohdan 1.1 esimerkissä.

# 1.3: ruskeat kirjaimet

Monulinäkymässä on hämäävää, jos keltaisella näkyy monta kertaa sama kirjain, vaikka sitä ei sanassa ole kuin kerran. Jos siis arvausrivillä on sama kirjain monta kertaa keltaisella, mutta yhdessäkään arvauksessa ei ole havaittu kirjainta niin montaa kertaa nimenomaan keltaisena, niin silloin kirjaimen tulee olla ruskea. Toisin sanoen monulinäkymässä useammin ei-vihreänä esiintyvä kirjain on ruskea, ellei ole ainakin yhtä arvausta, missä kirjain olisi ollut keltaisena yhtä monta kertaa.

Täytyy kuitenkin erottaa kaksi tilannetta: kirjain on nähty mustana taikka ei. Jos kirjain on nähty mustana, niin tiedetään, montako niitä voi enintään olla. Jos kirjain on nähty vihreänä/keltaisena muttei mustana, niin silloin niitä voi olla sanassa enemmänkin.

Semmoinen voisi vielä olla hyvä periaate, että vihreiden+keltaisten määrä näyttää, montako kirjaimia on, JOS niiden määrä tiedetään (eli on nähty kirjain myös mustana, koska se asettaa ylärajan havaitulle määrälle). Loput "keltaiset" olisivat sitten ruskeita. Jos taas ylärajaa ei ole tiedossa, silloin otetaan "esitettäväksi määräksi" se, montako kappaletta kirjainta on enintään nähty samassa arvauksessa. Siispä (enintään) niin monta vihreää+keltaista, loput ruskeita.

Huomattavaa on myös, että ennen kaikkea keltaisten+vihreiden suurimmalla havaitulla MÄÄRÄLLÄ yhdessä arvauksessa on väliä. Sen sijaan keltaiset ruudut eivät ole eri asia kuin mustat siltä kannalta, että niissä kirjainta ei ole. Kirjaimen sijainnin kannalta on vain kolmenlaisia ruutuja: vihreitä, keltaisia/mustia, sekä tuntemattomia (CharacterState).

Tarkennus: jos sanassa ei ole enää tuntemattomia ruutuja kirjaimelle X, niin silloin X:ää ei näytetä keltaisena/ruskeana ollenkaan. Logiikka on se, että X on silloin suljettu pois kaikista ei-vihreistä ruuduista, eikä sitä niin ollen voi enää esiintyä sanassa, jolloin sen näyttäminen olisi turhaa ja hämäävää.

**Esimerkki 1.3.1**: oikea sana LAHTI
- arvaus 1: KAALI (L väärässä paikassa 4)
- arvaus 2: PALVI (L väärässä paikassa 3)
- ennen 1.3-korjausta tiivistelmärivi olisi: tyhjä, vihreä A, keltainen L, keltainen L, vihreä I
- korjauksen jälkeen pitäisi tiivistelmärivin olla: tyhjä, vihreä A, ruskea L, ruskea L, vihreä I
- syy: L on kahdesti ei-vihreänä tiivistelmärivissä, mutta on vain arvauksia, missä on 1 keltainen L (vähemmän kuin 2)

Keltaisena tai ruskeana esittäminen on kirjainkohtaista niiden ruutujen joukossa, joissa ei ole vihreää. Toisten kirjainten värit vaikuttavat vain siten, että vihreät sulkevat pois kaikki muut kirjaimet kyseisestä ruudusta.

Käydään läpi tapauksia eri määrillä värejä arvausten ruuduissa:
- keltaisia 1, vihreitä 0, nähty 1, havaittu väärä: kirjainta on vain 1, näytetään keltainen joko keltaisessa ruudussa tai lisäruudussa (jos vihreän peittämä)
- keltaisia 1, vihreitä 1, nähty 1, havaittu väärä: kirjainta on vain 1, ei näytetä keltaista
- keltaisia 1, vihreitä 1, nähty 1, ei havaittu väärää: kirjainta on vähintään 1, näytetään ruskea keltaisessa ruudussa (tai ei jos vihreän peittämä)
- keltaisia 1, vihreitä 1, nähty 2, havaittu väärä: kirjainta on tasan 2, näytetään keltainen keltaisessa ruudussa tai lisäruudussa (jos vihreän peittämä)
- keltaisia 1, vihreitä 1, nähty 2, ei havaittu väärää: kirjainta on vähintään 2, näytetään keltainen keltaisessa ruudussa tai lisäruudussa (jos vihreän peittämä)
- keltaisia 2, vihreitä 0, nähty 1, havaittu väärä: kirjainta on tasan 1, näytetään yksi keltainen ja loput ruskeana

# 1.4: harmaat kirjaimet

Monulissa voisi silti olla paikka harmaille kirjaimille: jos TIEDETÄÄN, että kirjaimia on vain tietty määrä, ja silti on enemmän vääriä paikkoja kuin keltaisia. Tällöin ei ole järkeä näyttää ruskeaa vaan kannattaisi käyttää harmaata. Ruskea on tällöin merkitykseltään "ehkä sanassa" (maybe-present), harmaa olisi "ei sanassa" (absent).

Esimerkki 1.4.1: sana on PISIN
- arvaus: HIISI
- tulos monulirivissä olisi tällöin suunnilleen sama kuin sanulirivissä: tyhjä, vihreä I, keltainen I, keltainen S, harmaa I

# 1.5: monulin pisteytys: ei putki vaan vähimmät yritykset

Aiempi idea oli, että jokainen monulin sana pitäisi käydä erikseen ratkaisemassa. Kielimalli koodasi kuitenkin homma niin, että kokonaan vihreä sana oli ratkaistu (is_solved), vaikkei oikeaa sanaa olisi missään vaiheessa arvattu. Tämä tekee mahdolliseksi (jopa helpoksi) esim. Monuli(100):n ratkaisemisen 27 arvauksella. Tämä "bugi" johtaa oikeastaan paljon mielenkiintoisempaan ja taktisempaan monuliin!

Koska monulin ratkoja voi pyrkiä minimaaliseen määrään arvauksia avatakseen kaikki sanat, olisi ehkä järkevämpää seurata parasta tulosta (vähiten yrityksiä) putken sijaan. Monulista voisi toisin sanoen poistaa kaikki max_guesses ja streak -logiikat, mutta voi niiden toisaalta antaa olla niin kauan kuin ne ovat Game-traitissa.

# 2.0: cleanup sanuli-PR:ää varten

Poistetaan overview cursor, joka aiheutti kamalasti noisea main.rs:ään. Monuli-projektin tavoite on koskea Sanuli-koodiin mahdollisimman vähän. Poistetaan oikeastaan saman tien koko overview, ja työstetään sitä myöhemmin erillisessä branchissa, ehkä.

Uudellenkäytetään TileStatea jne. sekä samoja termejä: green -> correct, yellow -> present, brown -> maybe-present
