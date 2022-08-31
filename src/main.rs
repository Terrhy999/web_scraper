#![feature(async_closure)]
// use std::io::Error;

use chrono;
use futures::future::join_all;
use postgres::{Client, NoTls};
use thirtyfour::prelude::*;

#[tokio::main]
async fn scrape() -> Result<Vec<String>, WebDriverError> {
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
    let url_options = join_all(url_futures).await;

    println!("{:?}", url_options);

    // Always explicitly close the browser.
    driver.quit().await?;

    Ok(url_options)
}

fn main() {
    let mut client = Client::connect("host=/var/run/postgresql user=terrhy999 dbname=mydb", NoTls)
        .expect("I guess something broke");

    for row in client.query("SELECT * FROM weather", &[]).expect("panic 3") {
        dbg!(&row);
        let temp_lo: i32 = row.get("temp_lo");
        let temp_high: Option<i32> = row.get("temp_hi");
        let prcp: Option<f32> = row.get("prcp");
        let date: chrono::NaiveDate = row.get("date");

        println!("found person: {:?} {:?} {:?}", temp_lo, temp_high, prcp);
    }
}
