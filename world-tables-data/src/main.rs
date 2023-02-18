
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::Connection;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use world_tables_base::{Key, EntityLabel, Country, State, City, Currency, WorldRegion, WorldSubregion};
use world_tables_data::MIGRATIONS;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Loads all data into a local database file, creating the file if it doesn't exist
    #[clap(display_order = 1)]
    Local {
        /// Database file path
        #[arg(short, long, display_order = 1, value_name = "DB_FILE")]
        dbpath: Option<PathBuf>,
    },
    /// Uploads all the data through a server
    #[clap(display_order = 2)]
    Server {
        /// Server host address
        #[clap(short, long, display_order = 1, default_value_t = String::from("127.0.0.1"))]
        address: String,

        /// Server port
        #[clap(short, long, display_order = 2, default_value_t = 3000)]
        port: u16,
    },
}

impl Cli {
    fn execute(self) -> Result<()> {
        match self.command {
            Commands::Local { dbpath } => {
                let dbpath = if let Some(path) = dbpath {
                    path
                } else {
                    PathBuf::from("world.db3")
                };

                let mut conn = Connection::open(&dbpath).context("Could not open database file")?;

                conn.pragma_update(None, "journal_mode", "WAL")?;
                conn.pragma_update(None, "synchronous", "NORMAL")?;
                conn.pragma_update(None, "foreign_keys", "ON")?;

                MIGRATIONS.to_latest(&mut conn)?;

                let mut reader = csv::Reader::from_reader(include_str!("../data/countries.csv").as_bytes());

                let countries = reader
                    .deserialize()
                    .map(|result| {
                        let record: HashMap<String, String> = result?;
                        Ok(record)
                    })
                    .collect::<Result<Vec<HashMap<_, _>>>>()?;

                let currencies = countries
                    .iter()
                    .map(|rec| {
                        Currency {
                            iso: Key::new(rec["currency"].to_owned()),
                            name: rec["currency_name"].to_owned(),
                            symbol: rec["currency_symbol"].to_owned(),
                            ..Default::default()
                        }
                    })
                    .collect::<HashSet<_>>();

                for currency in currencies {
                    currency.save(&mut conn)?;
                }

                for record in &countries {
                    let region = match WorldRegion::key_with_name(&conn, &record["region"])? {
                        Key(None) => EntityLabel::None,
                        some => EntityLabel::KeyLabel(some, record["region"].to_owned()),
                    };

                    let subregion = match WorldSubregion::key_with_name(&conn, &record["subregion"])? {
                        Key(None) => EntityLabel::None,
                        some => EntityLabel::KeyLabel(some, record["subregion"].to_owned()),
                    };

                    let country = Country {
                        iso2: Key::new(record["iso2"].to_owned()),
                        iso3: record["iso3"].to_owned(),
                        name: record["name"].to_owned(),
                        code: record["numeric_code"].parse().context("Failed parsing numeric code")?,
                        capital: EntityLabel::KeyLabel(Key(None), record["capital"].to_owned()),
                        currency: EntityLabel::KeyLabel(Key::new(record["currency"].to_owned()), record["currency_name"].to_owned()),
                        tld: record["tld"].to_owned(),
                        native: record["native"].to_owned(),
                        region,
                        subregion,
                        latitude: record["latitude"].parse().context("Failed parsing country latitude")?,
                        longitude: record["longitude"].parse().context("Failed parsing country longitude")?,
                        emoji: record["emoji"].to_owned(),
                        emoji_u: record["emojiU"].to_owned(),
                        ..Default::default()
                    };

                    country.save(&mut conn).unwrap();
                }

                conn.execute("CREATE UNIQUE INDEX country_names ON countries(name);", []).unwrap();

                let mut reader = csv::Reader::from_reader(include_str!("../data/states.csv").as_bytes());

                for record in reader.deserialize() {
                    let record: HashMap<String, String> = record?;
                    let state = State {
                        name: record["name"].to_owned(),
                        country: EntityLabel::KeyLabel(Key::new(record["country_code"].to_owned()), record["country_name"].to_owned()),
                        code: record["state_code"].to_owned(),
                        latitude: record["latitude"].parse().ok(),
                        longitude: record["longitude"].parse().ok(),
                        ..Default::default()
                    };

                    state.save(&mut conn).unwrap();
                }

                conn.execute("CREATE INDEX state_names ON states(name);", []).unwrap();

                let mut reader = csv::Reader::from_reader(include_str!("../data/cities.csv").as_bytes());

                for record in reader.deserialize() {
                    let record: HashMap<String, String> = record?;
                    let state = match State::key_with_name(&conn, &record["state_name"])? {
                        Key(None) => EntityLabel::None,
                        some => EntityLabel::KeyLabel(some, record["state_name"].to_owned()),
                    };

                    let city = City {
                        name: record["name"].to_owned(),
                        state,
                        country: EntityLabel::KeyLabel(Key::new(record["country_code"].to_owned()), record["country_name"].to_owned()),
                        latitude: record["latitude"].parse().ok(),
                        longitude: record["longitude"].parse().ok(),
                        ..Default::default()
                    };

                    city.save(&mut conn).unwrap();
                }

                conn.execute("CREATE INDEX city_names ON cities(name);", []).unwrap();

                // Set the ids for capitals now that the cities table was filled
                for record in countries {
                    conn.execute(
                        "UPDATE countries SET capital_id = (SELECT id FROM cities WHERE cities.name = countries.capital AND cities.country_id = ?1)
                        WHERE iso2 = ?1;",
                        [&record["iso2"]]
                    ).unwrap();
                }
            }
            Commands::Server {..} => {
                todo!();
            }
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    Cli::parse().execute()
}
