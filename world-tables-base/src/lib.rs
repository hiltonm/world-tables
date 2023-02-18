
use anyhow::{Context, Result};
use rusqlite::{
    Connection,
    OptionalExtension,
    params,
    named_params,
};
use serde::{Serialize, Deserialize};
use url::Url;

pub use dbent::prelude::*;

pub trait Model {
    fn all(conn: &Connection, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> where Self: Sized;
    fn count(conn: &Connection) -> Result<usize>;
    fn get(conn: &Connection, key: &str) -> Result<Self> where Self: Sized;
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=========================  COUNTRY  ==========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Default, Debug, Entity, Label, Serialize, Deserialize)]
pub struct Country {
    pub iso2: Key<String>,
    pub iso3: String,
    #[label] pub name: String,
    pub code: u32,
    pub capital: EntityLabelInt<City>,
    pub currency: EntityLabelString<Currency>,
    pub tld: String,
    pub native: String,
    pub region: EntityLabelInt<WorldRegion>,
    pub subregion: EntityLabelInt<WorldSubregion>,
    pub latitude: f32,
    pub longitude: f32,
    pub emoji: String,
    pub emoji_u: String,
    pub states: Many<State>,
}

impl Model for Country {
    fn count(conn: &Connection) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM countries")
            .context("Failed preparing SQL for fetching countries count")?;

        stmt
            .query_row([], |row| {
                row.get(0)
            })
            .context("Failed querying countries count")
    }

    fn all(conn: &Connection, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn.prepare_cached(
                "SELECT iso2, name, world_region_id, world_region, world_subregion_id, world_subregion
                FROM countries
                LIMIT ?1
                OFFSET ?2")
            .context("Failed preparing SQL for fetching countries")?;
        let records = stmt
            .query_map([limit, offset], |row| {
                Ok(
                    Self {
                        iso2: row.get(0)?,
                        name: row.get(1)?,
                        region: EntityLabel::KeyLabel(row.get(2).unwrap_or_default(), row.get(3).unwrap_or_default()),
                        subregion: EntityLabel::KeyLabel(row.get(4).unwrap_or_default(), row.get(5).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::count(conn)?, records))
    }

    fn get(conn: &Connection, key: &str) -> Result<Self> {
        let mut stmt = conn.prepare_cached(
               "SELECT iso2, iso3, name, code, capital_id, capital, currency_id, currency,
               tld, native, world_region_id, world_region, world_subregion_id, world_subregion,
               latitude, longitude, emoji, emoji_u
               FROM countries
               WHERE iso2 = ?")
            .context("Failed preparing SQL for fetching country data")?;

        stmt
            .query_row([key], |row| {
                Ok(
                    Self {
                        iso2: row.get(0)?,
                        iso3: row.get(1)?,
                        name: row.get(2)?,
                        code: row.get(3)?,
                        capital: EntityLabel::KeyLabel(row.get(4)?, row.get(5).unwrap_or_default()),
                        currency: EntityLabel::KeyLabel(row.get(6)?, row.get(7).unwrap_or_default()),
                        tld: row.get(8)?,
                        native: row.get(9)?,
                        region: EntityLabel::KeyLabel(row.get(10).unwrap_or_default(), row.get(11).unwrap_or_default()),
                        subregion: EntityLabel::KeyLabel(row.get(12).unwrap_or_default(), row.get(13).unwrap_or_default()),
                        latitude: row.get(14)?,
                        longitude: row.get(15)?,
                        emoji: row.get(16)?,
                        emoji_u: row.get(17)?,
                        ..Default::default()
                    }
                )
            })
            .context("Failed querying country data")
    }
}

impl Country {
    pub fn save(&self, conn: &mut Connection) -> Result<()> {
        let Self {
            iso2,
            iso3,
            name,
            code,
            capital,
            currency,
            tld,
            native,
            region,
            subregion,
            latitude,
            longitude,
            emoji,
            emoji_u,
            ..
        } = self;

        conn.execute(
            "INSERT INTO countries
                (iso2, iso3, name, code, capital_id, capital, currency_id, currency,
                tld, native, world_region_id, world_region, world_subregion_id, world_subregion,
                latitude, longitude, emoji, emoji_u)
            VALUES
                (:iso2, :iso3, :name, :code, :capital_id, :capital, :currency_id, :currency,
                :tld, :native, :region_id, :region, :subregion_id, :subregion,
                :latitude, :longitude, :emoji, :emoji_u)
            ON CONFLICT(iso2) DO UPDATE
            SET
                iso3=:iso3, name=:name, code=:code, capital_id=:capital_id, capital=:capital,
                currency_id=:currency_id, currency=:currency, tld=:tld, native=:native,
                world_region_id=:region_id, world_region=:region, world_subregion_id=:subregion_id,
                world_subregion=:subregion, latitude=:latitude, longitude=:longitude, emoji=:emoji,
                emoji_u=:emoji_u;",
            named_params! {
                ":iso2": iso2,
                ":iso3": iso3,
                ":name": name,
                ":code": code,
                ":capital_id": capital.key().ok(),
                ":capital": capital.label().ok(),
                ":currency_id": currency.key().ok(),
                ":currency": currency.label().ok(),
                ":tld": tld,
                ":native": native,
                ":region_id": region.key().ok(),
                ":region": region.label().ok(),
                ":subregion_id": subregion.key().ok(),
                ":subregion": subregion.label().ok(),
                ":latitude": latitude,
                ":longitude": longitude,
                ":emoji": emoji,
                ":emoji_u": emoji_u,
            }
        )?;

        Ok(())
    }

    pub fn from_region(conn: &Connection, key: &str, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn
            .prepare_cached(
                "SELECT iso2, name, world_region_id, world_region, world_subregion_id, world_subregion
                FROM countries
                WHERE world_region_id = ?1
                LIMIT ?2
                OFFSET ?3")
            .context("Failed preparing SQL for fetching countries")?;

        let records = stmt
            .query_map(params![key, limit, offset], |row| {
                Ok(
                    Self {
                        iso2: row.get(0)?,
                        name: row.get(1)?,
                        region: EntityLabel::KeyLabel(row.get(2).unwrap_or_default(), row.get(3).unwrap_or_default()),
                        subregion: EntityLabel::KeyLabel(row.get(4).unwrap_or_default(), row.get(5).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::from_region_count(conn, key)?, records))
    }

    pub fn from_subregion(conn: &Connection, key: &str, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn
            .prepare_cached(
                "SELECT iso2, name, world_region_id, world_region, world_subregion_id, world_subregion
                FROM countries
                WHERE world_subregion_id = ?1
                LIMIT ?2
                OFFSET ?3")
            .context("Failed preparing SQL for fetching countries")?;

        let records = stmt
            .query_map(params![key, limit, offset], |row| {
                Ok(
                    Self {
                        iso2: row.get(0)?,
                        name: row.get(1)?,
                        region: EntityLabel::KeyLabel(row.get(2).unwrap_or_default(), row.get(3).unwrap_or_default()),
                        subregion: EntityLabel::KeyLabel(row.get(4).unwrap_or_default(), row.get(5).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::from_subregion_count(conn, key)?, records))
    }

    pub fn from_currency(conn: &Connection, key: &str, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn
            .prepare_cached(
                "SELECT iso2, name, world_region_id, world_region, world_subregion_id, world_subregion, currency_id, currency
                FROM countries
                WHERE currency_id = ?1
                LIMIT ?2
                OFFSET ?3")
            .context("Failed preparing SQL for fetching countries")?;

        let records = stmt
            .query_map(params![key, limit, offset], |row| {
                Ok(
                    Self {
                        iso2: row.get(0)?,
                        name: row.get(1)?,
                        region: EntityLabel::KeyLabel(row.get(2).unwrap_or_default(), row.get(3).unwrap_or_default()),
                        subregion: EntityLabel::KeyLabel(row.get(4).unwrap_or_default(), row.get(5).unwrap_or_default()),
                        currency: EntityLabel::KeyLabel(row.get(6).unwrap_or_default(), row.get(7).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::from_currency_count(conn, key)?, records))
    }

    pub fn from_region_count(conn: &Connection, key: &str) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM countries
            WHERE world_region_id = ?")
            .context("Failed preparing SQL for fetching countries count")?;

        stmt
            .query_row([key], |row| {
                row.get(0)
            })
            .context("Failed querying countries count")
    }

    pub fn from_subregion_count(conn: &Connection, key: &str) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM countries
            WHERE world_subregion_id = ?")
            .context("Failed preparing SQL for fetching countries count")?;

        stmt
            .query_row([key], |row| {
                row.get(0)
            })
            .context("Failed querying countries count")
    }

    pub fn from_currency_count(conn: &Connection, key: &str) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM countries
            WHERE currency_id = ?")
            .context("Failed preparing SQL for fetching countries count")?;

        stmt
            .query_row([key], |row| {
                row.get(0)
            })
            .context("Failed querying countries count")
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><========================  CURRENCY  ==========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Default, Debug, Entity, Label, Serialize, Deserialize)]
pub struct Currency {
    pub iso: Key<String>,
    #[label] pub name: String,
    pub symbol: String,
    pub countries: Many<Country>,
}

impl PartialEq for Currency {
    fn eq(&self, other: &Self) -> bool {
        self.iso == other.iso
    }
}

impl Eq for Currency {}

impl std::hash::Hash for Currency {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.iso.hash(state);
    }
}

impl Model for Currency {
    fn count(conn: &Connection) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM currencies")
            .context("Failed preparing SQL for fetching currencies count")?;

        stmt
            .query_row([], |row| {
                row.get(0)
            })
            .context("Failed querying currencies count")
    }

    fn all(conn: &Connection, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn.prepare_cached(
            "SELECT iso, name, symbol FROM currencies
            LIMIT ?1
            OFFSET ?2")
            .context("Failed preparing SQL for fetching currencies")?;
        let records = stmt
            .query_map([limit, offset], |row| {
                Ok(
                    Self {
                        iso: row.get(0)?,
                        name: row.get(1)?,
                        symbol: row.get(2)?,
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::count(conn)?, records))
    }

    fn get(conn: &Connection, key: &str) -> Result<Self> {
        let mut stmt = conn.prepare_cached(
           "SELECT iso, name, symbol FROM currencies
           WHERE iso = ?")
            .context("Failed preparing SQL for fetching currencies data")?;

        stmt
            .query_row([key], |row| {
                Ok(
                    Self {
                        iso: row.get(0)?,
                        name: row.get(1)?,
                        symbol: row.get(2)?,
                        ..Default::default()
                    }
                )
            })
            .context("Failed querying currencies data")
    }
}

impl Currency {
    pub fn save(&self, conn: &mut Connection) -> Result<()> {
        let Self {
            iso,
            name,
            symbol,
            ..
        } = self;

        let mut stmt = conn.prepare_cached("INSERT INTO currencies (iso, name, symbol) VALUES (?1, ?2, ?3)")?;
        stmt.execute(params![iso, name, symbol])?;

        Ok(())
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><======================  WORLD REGION  ========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Default, Debug, Entity, Label, Serialize, Deserialize)]
pub struct WorldRegion {
    pub id: Key<Int>,
    #[label] pub name: String,
    pub subregions: Many<WorldSubregion>,
    pub countries: Many<Country>,
}

impl Model for WorldRegion {
    fn count(conn: &Connection) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM world_regions")
            .context("Failed preparing SQL for fetching world regions count")?;

        stmt
            .query_row([], |row| {
                row.get(0)
            })
            .context("Failed querying world regions count")
    }

    fn all(conn: &Connection, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, name FROM world_regions
            LIMIT ?1
            OFFSET ?2")
            .context("Failed preparing SQL for fetching world regions")?;
        let records = stmt
            .query_map([limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::count(conn)?, records))
    }

    fn get(conn: &Connection, key: &str) -> Result<Self> {
        let mut stmt = conn.prepare_cached(
           "SELECT id, name FROM world_regions
           WHERE id = ?")
            .context("Failed preparing SQL for fetching world regions data")?;

        stmt
            .query_row([key], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        ..Default::default()
                    }
                )
            })
            .context("Failed querying world regions data")
    }
}

impl WorldRegion {
    pub fn key_with_name(conn: &Connection, name: &str) -> Result<Key<Int>> {
        let mut stmt = conn.prepare_cached(
            "SELECT id FROM world_regions
            WHERE name = ?")?;

        Ok(
            stmt
                .query_row([name], |row| {
                    row.get(0)
                })
                .optional()?
                .into()
        )
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><====================  WORLD SUBREGION  =======================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Default, Debug, Entity, Label, Serialize, Deserialize)]
pub struct WorldSubregion {
    pub id: Key<Int>,
    #[label] pub name: String,
    pub region: EntityLabelInt<WorldRegion>,
    pub countries: Many<Country>,
}

impl Model for WorldSubregion {
    fn count(conn: &Connection) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM world_subregions")
            .context("Failed preparing SQL for fetching world subregions count")?;

        stmt
            .query_row([], |row| {
                row.get(0)
            })
            .context("Failed querying world subregions count")
    }

    fn all(conn: &Connection, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn.prepare_cached(
            "SELECT sub.id, sub.name, sub.world_region_id, reg.name
            FROM world_subregions as sub
            LEFT JOIN world_regions as reg
            ON sub.world_region_id = reg.id
            LIMIT ?1
            OFFSET ?2")
            .context("Failed preparing SQL for fetching world subregions")?;

        let records = stmt
            .query_map([limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        region: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::count(conn)?, records))
    }

    fn get(conn: &Connection, key: &str) -> Result<Self> {
        let mut stmt = conn.prepare_cached(
           "SELECT sub.id, sub.name, sub.world_region_id, reg.name
           FROM world_subregions as sub
           LEFT JOIN world_regions as reg
           ON sub.world_region_id = reg.id
           WHERE sub.id = ?")
            .context("Failed preparing SQL for fetching world subregions data")?;

        stmt
            .query_row([key], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        region: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })
            .context("Failed querying world subregions data")
    }
}

impl WorldSubregion {
    pub fn key_with_name(conn: &Connection, name: &str) -> Result<Key<Int>> {
        let mut stmt = conn.prepare_cached(
            "SELECT id FROM world_subregions
            WHERE name = ?")?;

        Ok(
            stmt
                .query_row([name], |row| {
                    row.get(0)
                })
                .optional()?
                .into()
        )
    }

    pub fn from_region(conn: &Connection, key: &str, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn.prepare_cached(
            "SELECT sub.id, sub.name, sub.world_region_id, reg.name
            FROM world_subregions as sub
            LEFT JOIN world_regions as reg
            ON sub.world_region_id = reg.id
            WHERE reg.id = ?1
            LIMIT ?2
            OFFSET ?3")
            .context("Failed preparing SQL for fetching world subregions")?;

        let records = stmt
            .query_map(params![key, limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        region: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::from_region_count(conn, key)?, records))
    }

    pub fn from_region_count(conn: &Connection, key: &str) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM world_subregions
            WHERE world_region_id = ?")
            .context("Failed preparing SQL for fetching subregions count")?;

        stmt
            .query_row([key], |row| {
                row.get(0)
            })
            .context("Failed querying subregions count")
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=========================  STATE  ============================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Default, Debug, Entity, Label, Serialize, Deserialize)]
pub struct State {
    pub id: Key<Int>,
    #[label] pub name: String,
    pub code: String,
    pub country: EntityLabelString<Country>,
    pub latitude: Option<f32>,
    pub longitude: Option<f32>,
    pub cities: Many<City>,
}

impl Model for State {
    fn count(conn: &Connection) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM states")
            .context("Failed preparing SQL for fetching states count")?;

        stmt
            .query_row([], |row| {
                row.get(0)
            })
            .context("Failed querying states count")
    }

    fn all(conn: &Connection, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, country_id, country
            FROM states
            LIMIT ?1
            OFFSET ?2")
            .context("Failed preparing SQL for fetching states")?;
        let records = stmt
            .query_map([limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        country: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::count(conn)?, records))
    }

    fn get(conn: &Connection, key: &str) -> Result<Self> {
        let mut stmt = conn.prepare_cached(
           "SELECT id, name, code, country_id, country, latitude, longitude
           FROM states
           WHERE id = ?")
            .context("Failed preparing SQL for fetching state data")?;

        stmt
            .query_row([key], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        code: row.get(2)?,
                        country: EntityLabel::KeyLabel(row.get(3)?, row.get(4).unwrap_or_default()),
                        latitude: row.get(5)?,
                        longitude: row.get(6)?,
                        ..Default::default()
                    }
                )
            })
            .context("Failed querying state data")
    }
}

impl State {
    pub fn save(&self, conn: &mut Connection) -> Result<()> {
        let Self {
            id,
            name,
            code,
            country,
            latitude,
            longitude,
            ..
        } = self;

        conn.execute(
            "INSERT INTO states (id, name, code, country_id, country, latitude, longitude)
            VALUES (:id, :name, :code, :country_id, :country, :latitude, :longitude)
            ON CONFLICT(id) DO UPDATE
            SET
                name=:name,
                code=:code,
                country_id=:country_id,
                country=:country,
                latitude=:latitude,
                longitude=:longitude;",
            named_params! {
                ":id": id,
                ":name": name,
                ":code": code,
                ":country_id": country.key().ok(),
                ":country": country.label().ok(),
                ":latitude": latitude,
                ":longitude": longitude,
            }
        )?;

        Ok(())
    }

    pub fn key_with_name(conn: &Connection, name: &str) -> Result<Key<Int>> {
        let mut stmt = conn.prepare_cached(
            "SELECT id FROM states
            WHERE name = ?")?;

        Ok(
            stmt
                .query_row([name], |row| {
                    row.get(0)
                })
                .optional()?
                .into()
        )
    }

    pub fn from_country(conn: &Connection, key: &str, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, name, country_id, country FROM states
                WHERE country_id = ?1
                LIMIT ?2
                OFFSET ?3")
            .context("Failed preparing SQL for fetching states")?;

        let records = stmt
            .query_map(params![key, limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        country: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::from_country_count(conn, key)?, records))
    }

    pub fn from_country_count(conn: &Connection, key: &str) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM states
            WHERE country_id = ?")
            .context("Failed preparing SQL for fetching states count")?;

        stmt
            .query_row([key], |row| {
                row.get(0)
            })
            .context("Failed querying states count")
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=========================  CITY  =============================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Default, Debug, Entity, Label, Serialize, Deserialize)]
pub struct City {
    pub id: Key<Int>,
    #[label] pub name: String,
    pub state: EntityLabelInt<State>,
    pub country: EntityLabelString<Country>,
    pub latitude: Option<f32>,
    pub longitude: Option<f32>,
}

impl Model for City {
    fn count(conn: &Connection) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM cities")
            .context("Failed preparing SQL for fetching cities count")?;

        stmt
            .query_row([], |row| {
                row.get(0)
            })
            .context("Failed querying cities count")
    }

    fn all(conn: &Connection, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, state_id, state, country_id, country FROM cities
            LIMIT ?1
            OFFSET ?2")
            .context("Failed preparing SQL for fetching cities")?;
        let records = stmt
            .query_map([limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        state: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        country: EntityLabel::KeyLabel(row.get(4)?, row.get(5).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::count(conn)?, records))
    }

    fn get(conn: &Connection, key: &str) -> Result<Self> {
        let mut stmt = conn.prepare_cached(
           "SELECT * FROM cities
           WHERE id = ?")
            .context("Failed preparing SQL for fetching city data")?;

        stmt
            .query_row([key], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        state: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        country: EntityLabel::KeyLabel(row.get(4)?, row.get(5).unwrap_or_default()),
                        latitude: row.get(6)?,
                        longitude: row.get(7)?,
                    }
                )
            })
            .context("Failed querying city data")
    }
}

impl City {
    pub fn save(&self, conn: &mut Connection) -> Result<()> {
        let Self {
            id,
            name,
            state,
            country,
            latitude,
            longitude,
        } = self;

        conn.execute(
            "INSERT INTO cities (id, name, state_id, state, country_id, country, latitude, longitude)
            VALUES (:id, :name, :state_id, :state, :country_id, :country, :latitude, :longitude)
            ON CONFLICT(id) DO UPDATE
            SET
                name=:name,
                state_id=:state_id,
                state=:state,
                country_id=:country_id,
                country=:country,
                latitude=:latitude,
                longitude=:longitude;",
            named_params![
                ":id": id,
                ":name": name,
                ":state_id": state.key().ok(),
                ":state": state.label().ok(),
                ":country_id": country.key().ok(),
                ":country": country.label().ok(),
                ":latitude": latitude,
                ":longitude": longitude,
            ]
        )?;

        Ok(())
    }

    pub fn from_country(conn: &Connection, key: &str, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, name, state_id, state, country_id, country
                FROM cities
                WHERE country_id = ?1
                LIMIT ?2
                OFFSET ?3")
            .context("Failed preparing SQL for fetching cities")?;

        let records = stmt
            .query_map(params![key, limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        state: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        country: EntityLabel::KeyLabel(row.get(4)?, row.get(5).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::from_country_count(conn, key)?, records))
    }

    pub fn from_state(conn: &Connection, key: &str, limit: usize, offset: usize) -> Result<(usize, Vec<Self>)> {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, name, state_id, state, country_id, country
                FROM cities
                WHERE state_id = ?1
                LIMIT ?2
                OFFSET ?3")
            .context("Failed preparing SQL for fetching cities")?;

        let records = stmt
            .query_map(params![key, limit, offset], |row| {
                Ok(
                    Self {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        state: EntityLabel::KeyLabel(row.get(2)?, row.get(3).unwrap_or_default()),
                        country: EntityLabel::KeyLabel(row.get(4)?, row.get(5).unwrap_or_default()),
                        ..Default::default()
                    }
                )
            })?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        Ok((Self::from_state_count(conn, key)?, records))
    }

    pub fn from_country_count(conn: &Connection, key: &str) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM cities
            WHERE country_id = ?")
            .context("Failed preparing SQL for fetching cities count")?;

        stmt
            .query_row([key], |row| {
                row.get(0)
            })
            .context("Failed querying cities count")
    }

    pub fn from_state_count(conn: &Connection, key: &str) -> Result<usize> {
        let mut stmt = conn.prepare_cached(
            "SELECT count(*) FROM cities
            WHERE state_id = ?")
            .context("Failed preparing SQL for fetching cities count")?;

        stmt
            .query_row([key], |row| {
                row.get(0)
            })
            .context("Failed querying cities count")
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=========================  URL  ==============================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Debug)]
pub struct UrlBuilder {
    url: Url,
}

impl Default for UrlBuilder {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:3000".parse().unwrap(),
        }
    }
}

impl UrlBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_addr(addr: std::net::SocketAddr) -> Result<Self> {
        Ok(Self { url: format!("http://{}", &addr).parse()? })
    }

    pub fn with_base(host: &str) -> Self {
        Self {
            url: host.parse().unwrap(),
        }
    }

    pub fn as_str(&self) -> &str {
        self.url.as_ref()
    }
    // This builder is a bit different from normal ones
    // as the 'for' methods make clones of the base builder

    pub fn for_metadata(&self) -> Self {
        let mut builder = self.clone();
        builder.url.set_path("metadata");
        builder
    }

    pub fn for_country(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["country", key]);

        builder
    }

    pub fn for_state(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["state", key]);

        builder
    }

    pub fn for_city(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["city", key]);

        builder
    }

    pub fn for_world_region(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["region", key]);

        builder
    }

    pub fn for_world_subregion(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["subregion", key]);

        builder
    }

    pub fn for_currency(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["currency", key]);

        builder
    }

    pub fn for_countries(&self) -> Self {
        let mut builder = self.clone();
        builder.url.set_path("countries");
        builder
    }

    pub fn for_states(&self) -> Self {
        let mut builder = self.clone();
        builder.url.set_path("states");
        builder
    }

    pub fn for_cities(&self) -> Self {
        let mut builder = self.clone();
        builder.url.set_path("cities");
        builder
    }

    pub fn for_world_regions(&self) -> Self {
        let mut builder = self.clone();
        builder.url.set_path("regions");
        builder
    }

    pub fn for_world_subregions(&self) -> Self {
        let mut builder = self.clone();
        builder.url.set_path("subregions");
        builder
    }

    pub fn for_currencies(&self) -> Self {
        let mut builder = self.clone();
        builder.url.set_path("currencies");
        builder
    }

    pub fn for_countries_from_region(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["region", key, "countries"]);

        builder
    }

    pub fn for_countries_from_subregion(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["subregion", key, "countries"]);

        builder
    }

    pub fn for_countries_from_currency(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["currency", key, "countries"]);

        builder
    }

    pub fn for_states_from_country(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["country", key, "states"]);

        builder
    }

    pub fn for_cities_from_country(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["country", key, "cities"]);

        builder
    }

    pub fn for_cities_from_state(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["state", key, "cities"]);

        builder
    }

    pub fn for_subregions_from_region(&self, key: &str) -> Self {
        let mut builder = self.clone();
        builder
            .url
            .path_segments_mut()
            .unwrap()
            .extend(&["region", key, "subregions"]);

        builder
    }

    pub fn with_pagination(mut self, page: usize, limit: usize) -> Self {
        self.url
            .query_pairs_mut()
            .append_pair("page", &page.to_string())
            .append_pair("limit", &limit.to_string());
        self
    }

    pub fn build(self) -> String {
        self.url.into()
    }

    pub fn path(self) -> String {
        self.url.path().into()
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><======================  PROTOCOLS  ===========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub version: String,
    pub countries: usize,
    pub states: usize,
    pub cities: usize,
    pub regions: usize,
    pub subregions: usize,
    pub currencies: usize,
}
