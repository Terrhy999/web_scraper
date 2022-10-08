#![feature(async_closure)]

use futures::future::join_all;
use postgres::{types::Type, Client, NoTls};
use serde::{Deserialize, Serialize};
use serde_json;
use std::{fs, str::FromStr};
use thirtyfour::prelude::*;
use uuid::Uuid;

struct Card {
    card_id: String,
    oracle_id: String,
    name: String,
    mana_cost: Option<String>,
    cmc: f32,
    type_line: String,
    oracle_text: Option<String>,
    colors: Option<Vec<String>>,
    color_identity: Vec<String>,
    scryfall_uri: String,
    legal_brawl: bool,
    legal_historic: bool,
}

struct Deck {
    deck_id: Uuid,
    name: String,
    url: String,
    legal_brawl: bool,
    legal_historic: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct Legalaties {
    brawl: String,
    historicbrawl: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ScryfallCard {
    id: String,
    oracle_id: String,
    name: String,
    mana_cost: Option<String>,
    cmc: f32,
    type_line: String,
    oracle_text: Option<String>,
    colors: Option<Vec<String>>,
    color_identity: Vec<String>,
    scryfall_uri: String,
    legalities: Legalaties,
    layout: String,
}

struct ScrapedDeck {
    url: String,
    cards: Vec<Uuid>,
}

impl From<ScryfallCard> for Card {
    fn from(c: ScryfallCard) -> Self {
        Self {
            card_id: c.id,
            oracle_id: c.oracle_id,
            name: c.name,
            mana_cost: c.mana_cost,
            cmc: c.cmc,
            type_line: c.type_line,
            oracle_text: c.oracle_text,
            colors: c.colors,
            color_identity: c.color_identity,
            scryfall_uri: c.scryfall_uri,
            legal_brawl: match c.legalities.brawl.as_str() {
                "legal" => true,
                _ => false,
            },
            legal_historic: match c.legalities.historicbrawl.as_str() {
                "legal" => true,
                _ => false,
            },
        }
    }
}

const AETHERHUB_URL: &str = "https://aetherhub.com";

// BROKEN!!!
// It's scraping links to users as well as decks
#[tokio::main]
async fn scrape_aetherhub_urls() -> Result<Vec<String>, WebDriverError> {
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:9515", caps).await?;

    // Navigate to https://aetherhub.com/Decks/Historic-Brawl/.
    driver
        .goto("https://aetherhub.com/Decks/Historic-Brawl/")
        .await?;

    // Find the table of decks
    let deck_table = driver.find(By::Id("metaHubTable")).await?;

    // Find table body
    let table_body = deck_table.find(By::Tag("tbody")).await?;

    // Get a vector of all the "a" elements in the table
    let table_link_elements = table_body.find_all(By::Tag("a")).await?;

    // Map over the elements getting the 'href' attribute, collecting back to a vector of futures
    let url_futures = table_link_elements
        .iter()
        .map(async move |x| {
            let url_option = x.attr("href").await.unwrap();
            match url_option {
                None => String::from("whoops no url"),
                Some(url) => url,
            }
        })
        .collect::<Vec<_>>();

    // Squish all the futures into one future I guess?
    let url_options: Vec<String> = join_all(url_futures).await;

    let deck_urls: Vec<String> = url_options
        .into_iter()
        .filter(|url| url.starts_with("/Deck"))
        .collect();

    // Always explicitly close the browser.
    driver.quit().await?;

    Ok(deck_urls)
}

#[tokio::main]
async fn get_decklists(deck_urls: Vec<String>) -> Vec<ScrapedDeck> {
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:9515", caps).await.unwrap();

    let mut decks: Vec<ScrapedDeck> = Vec::new();

    for deck_url in deck_urls.into_iter() {
        let full_url = format!("{}{}", AETHERHUB_URL, deck_url);

        println!("Going to {}", deck_url);
        driver
            .goto(full_url)
            .await
            .expect("couldn't navigate to url");

        let visual_tab = driver
            .find(By::Css("div[id^=tab_visual]"))
            .await
            .expect("couldn't find visual tab");

        let card_elements = visual_tab
            .find_all(By::ClassName("cardLink"))
            .await
            .expect("card-containers broke");

        let card_futures = card_elements
            .iter()
            .map(async move |card| {
                let card_name_option = card.attr("data-card-name").await.expect("get card name");
                match card_name_option {
                    None => String::from("no card name"),
                    Some(name) => name,
                }
            })
            .collect::<Vec<_>>();

        let card_strings = join_all(card_futures).await;

        let cards = get_card_ids(card_strings).await;

        println!("Found {} cards", cards.len());
        decks.push(ScrapedDeck {
            url: deck_url,
            cards,
        });
    }
    driver.quit().await.unwrap();
    decks
}

#[tokio::main]
async fn get_aetherhub_decklist(deck_url: &str) -> Vec<String> {
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:9515", caps)
        .await
        .expect("couldn't connect to chromedriver");

    let full_url = format!("{}{}", AETHERHUB_URL, deck_url);

    driver
        .goto(full_url)
        .await
        .expect("couldn't navigate to url");

    let visual_tab = driver
        .find(By::Css("div[id^=tab_visual]"))
        .await
        .expect("couldn't find visual tab");

    let card_elements = visual_tab
        .find_all(By::ClassName("cardLink"))
        .await
        .expect("card-containers broke");

    let card_futures = card_elements
        .iter()
        .map(async move |card| {
            let card_name_option = card.attr("data-card-name").await.unwrap();
            match card_name_option {
                None => String::from("no card name"),
                Some(name) => name,
            }
        })
        .collect::<Vec<_>>();

    let cards = join_all(card_futures).await;

    cards.iter().for_each(|x| println!("{}", x));

    println!("found {} cards", cards.len());

    driver.quit().await.expect("couldn't quit?");

    cards
}

fn get_legal_cards(path: &str) -> Vec<Card> {
    let data = fs::read_to_string(path).expect("unable to read JSON");
    let scryfall_cards: Vec<ScryfallCard> =
        serde_json::from_str(&data).expect("unable to parse JSON");

    let filtered_cards: Vec<ScryfallCard> = scryfall_cards
        .into_iter()
        .filter(|card| match card.layout.as_str() {
            "planar" => false,
            "scheme" => false,
            "vanguard" => false,
            "token" => false,
            "double_faced_token" => false,
            "emblem" => false,
            "augment" => false,
            "host" => false,
            "art_series" => false,
            "reversible_card" => false,
            _ => true,
        })
        .collect();

    let cards: Vec<Card> = filtered_cards
        .into_iter()
        .map(|c| {
            let card = Card::from(c);
            card
        })
        .collect();
    cards
}

fn create_card_table() {
    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let create_query = "
    CREATE TABLE card (
        card_id uuid NOT NULL PRIMARY KEY,
        oracle_id uuid NOT NULL,
        name text NOT NULL,
        mana_cost text,
        cmc real NOT NULL,
        type_line text NOT NULL,
        oracle_text text,
        colors char(1)[],
        color_identity char(1)[] NOT NULL,
        scryfall_uri text NOT NULL,
        legal_brawl bool NOT NULL,
        legal_historic bool NOT NULL)
    ";

    client
        .batch_execute(create_query)
        .expect("create table broke");
}

fn populate_card_table() {
    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let cards = get_legal_cards("src/oracle-cards-20220906090223.json");

    let insert_statement = client.prepare_typed("
    INSERT INTO card (card_id, oracle_id, name, mana_cost, cmc, type_line, oracle_text, colors, color_identity, scryfall_uri, legal_brawl, legal_historic) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)", &[Type::UUID, Type::UUID]).expect("insert statement broke");

    for card in cards {
        client
            .execute(
                &insert_statement,
                &[
                    &Uuid::parse_str(&card.card_id).expect("uuid parsed wrong"),
                    &Uuid::parse_str(&card.oracle_id).expect("uuid broke"),
                    &card.name,
                    &card.mana_cost,
                    &card.cmc,
                    &card.type_line,
                    &card.oracle_text,
                    &card.colors,
                    &card.color_identity,
                    &card.scryfall_uri,
                    &card.legal_brawl,
                    &card.legal_historic,
                ],
            )
            .expect("whoopsie");
        println!("{}", card.name)
    }
}

fn create_deck_table() {
    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let create_table_query = "
    CREATE TABLE deck (
        deck_id uuid NOT NULL PRIMARY KEY,
        url text NOT NULL)
    ";

    client
        .batch_execute(create_table_query)
        .expect("create table broke");
}

fn populate_deck_table() {
    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let populate_table_query = client
        .prepare_typed(
            "INSERT INTO deck (deck_id, name, url, format) VALUES ($1, $2, $3, $4)",
            &[Type::UUID],
        )
        .expect("deck query broke");

    client
        .execute(
            &populate_table_query,
            &[
                &Uuid::new_v4(),
                &"test name",
                &"https://aetherhub.com/Deck/liliana-of-the-veil-black-control",
                &"brawl",
            ],
        )
        .expect("populate table broke");
}

fn create_card_deck_table() {
    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let create_table_query = "
    CREATE TABLE card_deck (
        card_id uuid REFERENCES card (card_id),
        deck_id uuid REFERENCES deck (deck_id))
    ";

    client
        .batch_execute(create_table_query)
        .expect("create table broke");
}

fn populate_card_deck_table() {
    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let query = "INSERT INTO card_deck (card_id, deck_id) VALUES ($1, $2)";

    let create_deck = client
        .prepare_typed(query, &[Type::UUID])
        .expect("prepared query broke");

    client
        .execute(
            &create_deck,
            &[
                &Uuid::from_str("280f2a85-1900-460b-a768-164fc2dea636").expect("broke"),
                &Uuid::from_str("a17c6086-2b4e-4974-a67a-0ddc4d321c43").expect("broke"),
            ],
        )
        .expect("populate card_deck broke");
}

async fn get_card_ids(cards: Vec<String>) -> Vec<Uuid> {
    fn flip_query(card: &String) -> String {
        let mut query = "%".to_string();
        query.push_str(card);
        query.push('%');
        query
    }

    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let card_ids = cards
        .iter()
        .map(|card| {
            let result = client.query_one("SELECT card_id FROM card WHERE name = $1", &[card]);

            let row = match result {
                Ok(r) => r,
                Err(_) => {
                    let param = flip_query(card);
                    let result =
                        client.query_one("SELECT card_id FROM card WHERE name LIKE $1", &[&param]);

                    let row = match result {
                        Ok(r) => r,
                        Err(_) => panic!("Couldn't find {}", card),
                    };
                    row
                }
            };

            let card_id = row.get("card_id");
            card_id
        })
        .collect::<Vec<Uuid>>();

    card_ids
}

fn add_deck(deck_url: &str, card_ids: Vec<Uuid>) {
    let deck_uuid = Uuid::new_v4();

    let mut client = Client::connect(
        "host=/var/run/postgresql user=terrhy999 dbname=brawl_hub",
        NoTls,
    )
    .expect("I guess something broke");

    let create_deck_query = client
        .prepare_typed(
            "INSERT INTO deck (deck_id, url) VALUES ($1, $2)",
            &[Type::UUID],
        )
        .unwrap();

    client
        .execute(&create_deck_query, &[&deck_uuid, &deck_url])
        .unwrap();

    let populate_deck_query = client
        .prepare_typed(
            "INSERT INTO card_deck (deck_id, card_id) VALUES ($1, $2)",
            &[Type::UUID],
        )
        .unwrap();

    card_ids.iter().for_each(|card_id| {
        client
            .execute(&populate_deck_query, &[&deck_uuid, &card_id])
            .unwrap();
    })
}

fn main() {
    let urls = scrape_aetherhub_urls().unwrap();

    let decks = get_decklists(urls);

    // decks
    //     .into_iter()
    //     .for_each(|deck| add_deck(&deck.url, deck.cards))
}
