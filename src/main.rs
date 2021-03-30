use anyhow::Error;
use csv::Writer;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{env, fs, io};

#[derive(Debug, Serialize, Deserialize)]
struct GeoDataAddress {
    status: String,
    results: Vec<AddressResult>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddressResult {
    address_components: Vec<AddressComponent>,
    formatted_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddressComponent {
    long_name: String,
    short_name: String,
    types: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct AddressOutput {
    address: String,
    township: String,
}

/// Gets a path to a file containing a list of addresses separated by a newline.
fn read_addresses() -> Result<Vec<String>, Error> {
    let mut path = String::new();

    print!("Please input path: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut path)?;

    let contents = fs::read_to_string(path.trim())?;
    let addresses: Vec<String> = contents.split("\n").map(|s| s.to_string()).collect();
    println!("Successfully read your list of addresses!");

    Ok(addresses)
}

/// Uses the Google GeoCode API to find geo data from a list of addresses.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let fake_addr = String::from("123 Fake Addr Rd, Springfield, OH");
/// let geo_data = get_geo_data(vec![fake_addr]);
/// ```
async fn get_geo_data(addresses: Vec<String>) -> Result<Vec<GeoDataAddress>, Error> {
    let api_key = env::var("API_KEY")?;
    let base_url = "https://maps.googleapis.com/maps/api/geocode/json";
    let mut results: Vec<GeoDataAddress> = vec![];
    let client = reqwest::Client::new();

    println!("Starting data gathering...");

    for address in addresses {
        println!("Processing {}", address);

        let response: GeoDataAddress = client
            .get(base_url)
            .query(&[("key", &api_key), ("address", &address)])
            .send()
            .await?
            .json()
            .await?;

        results.push(response);
    }

    Ok(results)
}

/// Finds the correct township in a `GeoDataAddress`.
///
/// This function uses the first result in a given `GeoDataAddress`. It checks a couple different
/// types of political entities, in the order of more to less specific. A locality takes precedence over a level 3
/// area, which takes precedence over level 2. Returns an `Option<String>`, representing either the township or a failure to parse.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let fake_addr = String::from("123 Fake Addr Rd, Springfield, OH");
/// let geo_data = get_geo_data(vec![fake_addr]);
/// let township = get_township(geo_data); // "Springfield City"
/// ```
fn get_township(result: GeoDataAddress) -> Option<(String, String)> {
    if result.status != "OK" {
        return None;
    }

    let mut township = String::new();
    let first_result = result.results.get(0)?;

    for addr_component in &first_result.address_components {
        if addr_component.types.iter().any(|t| t == "locality") {
            township = addr_component.long_name.clone();
            break;
        }

        if addr_component.types.iter().any(|t| t == "administrative_area_level_3") {
            township = addr_component.long_name.clone();
            break;
        }

        if addr_component.types.iter().any(|t| t == "administrative_area_level_2") {
            township = addr_component.long_name.clone();
            break;
        }
    }

    if township == "Springfield" {
        township = String::from("Springfield City");
    }

    println!("Township for {} is {}", &first_result.formatted_address, township);

    return Some((first_result.formatted_address.clone(), township));
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    if let Err(_) = fs::remove_file("output.csv") {
        println!("Tried to remove output.csv, but didn't exist. Fine!");
    }

    if let Ok(addrs) = read_addresses() {
        let results = get_geo_data(addrs).await?;
        println!("Successfully got {} results!", results.len());

        let mut csv_writer = Writer::from_path("output.csv")?;
        csv_writer.write_record(&["Address", "Township"])?;

        for addr in results {
            if let Some((full_address, township)) = get_township(addr) {
                csv_writer.write_record(&[full_address, township])?;
            }
        }

        csv_writer.flush()?;
    }

    Ok(())
}
