
use anyhow::{Context, Result};
use enum_map::Enum;
use reqwest::blocking::Response;
use reqwest::header::HeaderMap;

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><========================  DATAKIND  ==========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone, Copy, Debug, Enum)]
pub(crate) enum MainList {
    Countries,
    States,
    Cities,
    Regions,
    Subregions,
    Currencies,
}

#[derive(Debug)]
pub(crate) enum MainListData<'a> {
    Countries(&'a str, Option<Pagination>),
    States(&'a str, Option<Pagination>),
    Cities(&'a str, Option<Pagination>),
    Regions(&'a str, Option<Pagination>),
    Subregions(&'a str, Option<Pagination>),
    Currencies(&'a str, Option<Pagination>),
}

impl<'a> MainListData<'a> {
    pub fn column_headers(&self) -> &[&'static str] {
        match self {
            MainListData::Countries(..) => &["Country", "Region", "Subregion"],
            MainListData::States(..) => &["State", "Country"],
            MainListData::Cities(..) => &["City", "State", "Country"],
            MainListData::Regions(..) => &["Region"],
            MainListData::Subregions(..) => &["Subregion", "Region"],
            MainListData::Currencies(..) => &["Name", "ISO", "Symbol"],
        }
    }

    pub fn data(&self) -> (&'a str, Option<Pagination>) {
        match self {
            MainListData::Countries(title, pagination) => (title, *pagination),
            MainListData::States(title, pagination) => (title, *pagination),
            MainListData::Cities(title, pagination) => (title, *pagination),
            MainListData::Regions(title, pagination) => (title, *pagination),
            MainListData::Subregions(title, pagination) => (title, *pagination),
            MainListData::Currencies(title, pagination) => (title, *pagination),
        }
    }
}

#[derive(Clone, Copy, Debug, Enum)]
pub(crate) enum DataKind {
    Metadata,

    Countries,
    States,
    Cities,
    Regions,
    Subregions,
    Currencies,

    Country,
    State,
    City,
    Region,
    Subregion,
    Currency,

    CountriesByRegion,
    CountriesBySubregion,
    CountriesByCurrency,
    StatesByCountry,
    CitiesByCountry,
    CitiesByState,
    SubregionsByRegion,
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=======================  DATA TYPES  =========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Debug)]
pub(crate) struct DataResponse {
    pub response: Response,
    pub pagination: Option<Pagination>,
    pub counts: Option<Counts>,
    pub page_text: String,
}

#[derive(Default, Debug)]
pub(crate) struct TableData<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
    pub page_text: String,
}

impl<T: serde::de::DeserializeOwned> From<DataResponse> for Option<TableData<T>> {
    fn from(data_response: DataResponse) -> Self {
        let option_data = data_response.response.json().ok();
        option_data.map(|data|
            TableData {
                data,
                pagination: data_response.pagination.unwrap(),
                page_text: data_response.page_text,
            }
        )
    }
}


#[derive(Default, Debug)]
pub(crate) struct FilteredTableData<T> {
    pub data: Option<TableData<T>>,
    pub show: bool,
    pub title: String,
}

#[derive(Clone, Debug)]
pub(crate) enum ServerData<T> {
    Ok(T),
    Loading,
    Failed(String, f64), // error message with time for delay
    Empty,
}

impl<T> ServerData<T> {
    pub fn unwrap_ref(&self) -> &T {
        match self {
            ServerData::Ok(data) => data,
            _ => panic!("Failed unwrap on server data"),
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, ServerData::Ok(_))
    }
}

#[derive(Debug)]
pub(crate) struct ObjectData<T> {
    pub data: Option<T>,
    pub show: bool,
    pub title: String,
    pub counts: Option<Counts>,
}

impl<T> Default for ObjectData<T> {
    fn default() -> Self {
        Self {
            data: None,
            show: true,
            title: Default::default(),
            counts: None,
        }
    }
}

impl<T: serde::de::DeserializeOwned> From<DataResponse> for Option<T> {
    fn from(data_response: DataResponse) -> Self {
        data_response.response.json().ok()
    }
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=====================  HEADER HANDLERS  ======================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[allow(dead_code)] // for limit
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Pagination {
    pub count: usize,
    pub total_count: usize,
    pub page: usize,
    pub limit: usize,
    pub total_pages: usize,
}

impl Pagination {
    pub fn with_headers(headers: &HeaderMap) -> Result<Pagination> {
        Ok(
            Pagination {
                count: headers
                    .get("Pagination-Count")
                    .context("Pagination-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header pagination count number")?,
                total_count: headers
                    .get("Pagination-Total-Count")
                    .context("Pagination-Total-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header pagination total count number")?,
                page: headers
                    .get("Pagination-Page")
                    .context("Pagination-Page header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header pagination page number")?,
                limit: headers
                    .get("Pagination-Limit")
                    .context("Pagination-Limit header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header pagination limit number")?,
                total_pages: headers
                    .get("Pagination-Total-Pages")
                    .context("Pagination-Total-Pages header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header pagination total pages number")?,
            }
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Counts {
    Country { states: usize, cities: usize },
    State { cities: usize },
    Region { countries: usize, subregions: usize },
    Subregion { countries: usize },
    Currency { countries: usize },
}

impl Counts {
    pub fn with_country_headers(headers: &HeaderMap) -> Result<Self> {
        Ok(
            Self::Country {
                states: headers
                    .get("States-Count")
                    .context("States-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header states count number")?,
                cities: headers
                    .get("Cities-Count")
                    .context("Cities-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header cities count number")?,
            }
        )
    }

    pub fn with_state_headers(headers: &HeaderMap) -> Result<Self> {
        Ok(
            Self::State {
                cities: headers
                    .get("Cities-Count")
                    .context("Cities-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header cities count number")?,
            }
        )
    }

    pub fn with_region_headers(headers: &HeaderMap) -> Result<Self> {
        Ok(
            Self::Region {
                countries: headers
                    .get("Countries-Count")
                    .context("Countries-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header countries count number")?,
                subregions: headers
                    .get("Subregions-Count")
                    .context("Subregions-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header subregions count number")?,
            }
        )
    }

    pub fn with_subregion_headers(headers: &HeaderMap) -> Result<Self> {
        Ok(
            Self::Subregion {
                countries: headers
                    .get("Countries-Count")
                    .context("Countries-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header countries count number")?,
            }
        )
    }

    pub fn with_currency_headers(headers: &HeaderMap) -> Result<Self> {
        Ok(
            Self::Currency {
                countries: headers
                    .get("Countries-Count")
                    .context("Countries-Count header not present")?
                    .to_str()?
                    .parse()
                    .context("Could not parse header countries count number")?,
            }
        )
    }
}

