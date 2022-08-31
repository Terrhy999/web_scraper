#![feature(async_closure)]
use futures::future::join_all;
use thirtyfour::prelude::*;

#[tokio::main]
pub async fn scrape() -> WebDriverResult<()> {
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

  Ok(())
}