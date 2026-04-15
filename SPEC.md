# spec version 1.0: monuli

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
	- keltaiset täytetään vasemmalta oikealle ei-vihreisiin ruutuihin
	- loput ruudut ovat mustia ilman kirjainta
- voiko niin käydä ettei keltaiset mahdu?
	- keltaisia ei näytetä, jos on jo vihreä tarjolla; vaikka tietysti ei tiedetä, onko sanassa toinen sama kirjain, jolloin keltainen olisi ok näyttää
	- tällä tavoin sanan esitys mahtuu 5 (tai 6) laatikkoon, jos mustia ei näytetä; vihreitä plus keltaisia ei voi olla enempää kuin sanassa on kirjaimia koska keltaisia on enintään sanan kirjainten verran, ja jokainen vihreä syö pois joko yhden keltaisen tai yhden mustan ruudun
	- EI VOI käydä siis
- entä tilanne, jossa 4 vihreää on löytynyt, ja jäljelle jääväkin kirjain on löytynyt mutta keltainen
	- silloin se on piirrettävä keltaisella ainoalle jäljelle jäävälle paikalle, vaikkei siinä enää ole arvattavaa
	- EI HAITTAA, ja saattaahan sana tulla valmiiksi muutenkin vahingossa, kun keltaiset piirretään vahingossa paikoilleen
- entä tilanne, jossa 5 vihreää on löytynyt, mutta ne eivät ole samassa arvauksessa, ts. sanaa ei ole arvattu mutta se näkyisi monulinäkymässä kokonaan vihreänä
	- EI HAITTAA sinänsä, mutta selvästikin kokonaan arvatut sanat pitää ehkä esittää niin että erottuu tuo tilanne?
- kokonaan arvatut sanat siirretään monulinäkymän hännille siinä järjestyksessä kuin ne arvattiin
	- tällöin viiden vihreän sanat, joita kuitenkaan ei ole arvattu, näkyvät monulilistan kärjessä, kun taas arvatut sanat ovat viimeisen ei-täysin-vihreän sanan jälkeen
- lisäratkaisu: rendataan erotin (esim. täysin musta rivi) ennen ratkaistujen osiota
	- monulilistasta tulee tällöin N+erotin pituinen, ei haittaa

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

# tarkennuksia: 1.1

Monulinäkymässä pitäisi näkyä, missä kohdissa vääriin kohtiin arvatut kirjaimet ovat. Samaan kohtaan voi tulla useita vääriä arvauksia:
- esimerkki 1.1.1: oikea sana LAHTI
    - arvaus 1: KAALI (L väärässä paikassa 4)
    - arvaus 2: TARHA (H väärässä paikassa 4)
Silloin pitäisi samassa ruudussa olla useampi keltainen kirjain, kuitenkin niitä voi olla maksimissaan neljä viiden kirjaimen sanassa (viimeistään viides olisi oikein). Siispä jos samassa kohdassa on useampi keltainen kirjain, ne pitää merkitä pienemmällä. Yksi mahdollisuus olisi tehdä niistä neljänneksen kokoisia (puolikas korkeus ja leveys).

Kukin monulinäkymän arvausrivi on siis viisi (tai kuusi) kappaletta jotain seuraavista:
- tyhjä ruutu
- vihreätaustainen kirjain
- keltataustainen kirjain
- 2x2 ruudukko, jossa:
    - keltataustainen kirjain
    - tyhjä ruutu

esimerkin 1.1.1 kahden arvauksen jälkeen väritys olisi:
- keltataustainen T
- vihreätaustainen A
- tyhjä ruutu
- 2x2 ruudukko, jossa:
    - keltataustainen L
    - keltataustainen H
- vihreätaustainen I
