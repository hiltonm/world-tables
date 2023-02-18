
use anyhow::{bail, Context, Result};
use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    extract::{Path, Query},//FromRequestParts,
    Extension,
    Router,
    Json,
};
use directories::ProjectDirs;
use log::{info, debug};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use serde::Deserialize;
use std::{
    net::TcpListener,
    process::Command,
    path::PathBuf,
    env,
    thread,
    time,
};
use tokio::signal;
use tower_http::compression::CompressionLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use world_tables_base::{Model, Country, State, City, WorldRegion, WorldSubregion, Currency, UrlBuilder, Metadata};
use world_tables_data::MIGRATIONS;

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><==========================  MAIN  ============================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "world_tables_server=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut db_path = ProjectDirs::from("", "", "world-tables")
        .expect("no valid home directory path could be retrieved from the operating system")
        .data_local_dir()
        .to_path_buf();

    std::fs::create_dir_all(&db_path)?;

    db_path.push("world.db3");
    debug!("database path: {:?}", &db_path);

    let mut work_dir = std::env::current_exe().expect("could not find current exe path");
    work_dir.pop();
    debug!("working dir: {:?}", &work_dir);

    if !db_path.try_exists().expect("can't check existence of database file") {
        let output = Command::new("./world-tables-data")
            .current_dir(&work_dir)
            .arg("local")
            .arg("-d")
            .arg(&db_path)
            .output()
            .expect("failed to execute database creation process");

        if !output.status.success() {
            bail!("database creation process failed");
        }
    }

    let url = UrlBuilder::new();

    let app = Router::new()
        .route("/", get(api_index))
        .route(&url.for_metadata().path(), get(metadata))

        .route(&url.for_countries().path(), get(countries_index))
        .route(&url.for_states().path(), get(states_index))
        .route(&url.for_cities().path(), get(cities_index))
        .route(&url.for_world_regions().path(), get(world_regions_index))
        .route(&url.for_world_subregions().path(), get(world_subregions_index))
        .route(&url.for_currencies().path(), get(currencies_index))

        .route(&url.for_country(":key").path(), get(country_data))
        .route(&url.for_state(":key").path(), get(state_data))
        .route(&url.for_city(":key").path(), get(city_data))
        .route(&url.for_world_region(":key").path(), get(region_data))
        .route(&url.for_world_subregion(":key").path(), get(subregion_data))
        .route(&url.for_currency(":key").path(), get(currency_data))

        .route(&url.for_countries_from_region(":key").path(), get(countries_from_region))
        .route(&url.for_countries_from_subregion(":key").path(), get(countries_from_subregion))
        .route(&url.for_countries_from_currency(":key").path(), get(countries_from_currency))
        .route(&url.for_states_from_country(":key").path(), get(states_from_country))
        .route(&url.for_cities_from_country(":key").path(), get(cities_from_country))
        .route(&url.for_cities_from_state(":key").path(), get(cities_from_state))
        .route(&url.for_subregions_from_region(":key").path(), get(subregions_from_region))

        .layer(init_db(db_path)?)
        .layer(CompressionLayer::new());

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    //let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Listening on {}", &addr);

    thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(1500));

        let _ = Command::new("./world-tables-gui")
            .current_dir(work_dir)
            .arg("-a")
            .arg(addr.to_string())
            .output()
            .expect("failed launching GUI app");

        #[cfg(unix)]
        Command::new("kill")
            .arg("-SIGTERM")
            .arg(std::process::id().to_string())
            .spawn()
            .expect("failed killing the server");

        #[cfg(windows)]
        Command::new("taskkill")
            .arg("/F")
            .arg("/PID")
            .arg(std::process::id().to_string())
            .spawn()
            .expect("failed killing the server");
    });

    axum::Server::from_tcp(listener)?
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><========================  HANDLERS  ==========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Debug, Deserialize)]
pub struct Pagination {
    pub page: usize,
    pub limit: usize,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            limit: 10
        }
    }
}

impl Pagination {
    pub fn to_limit_offset(&self) -> (usize, usize) {
        (self.limit, self.page.saturating_sub(1) * self.limit)
    }
}

fn pagination_headers(pagination: Pagination, count: usize, total_count: usize) -> HeaderMap {
    let mut headers = HeaderMap::with_capacity(5);
    headers.insert("Pagination-Count", count.into());
    headers.insert("Pagination-Total-Count", total_count.into());
    headers.insert("Pagination-Page", pagination.page.into());
    headers.insert("Pagination-Limit", pagination.limit.into());
    headers.insert("Pagination-Total-Pages", ((total_count as f32 / pagination.limit as f32).ceil() as usize).into());
    headers
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><====================  INDEX HANDLERS  ========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

async fn api_index() -> impl IntoResponse {
    "World tables API"
}

async fn metadata(Extension(db): Extension<Database>) -> Result<impl IntoResponse, AppError> {
    let conn = db.connection()?;

    let meta = Metadata {
        version: env!("CARGO_PKG_VERSION").to_string(),
        countries: Country::count(&conn)?,
        states: State::count(&conn)?,
        cities: City::count(&conn)?,
        regions: WorldRegion::count(&conn)?,
        subregions: WorldSubregion::count(&conn)?,
        currencies: Currency::count(&conn)?,
    };

    Ok(Json(meta))
}

async fn index<T>(db: Database, pagination: Option<Query<Pagination>>) -> Result<impl IntoResponse, AppError>
where
    T: Model + serde::ser::Serialize
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = T::all(&*db.connection()?, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

async fn countries_index(pagination: Option<Query<Pagination>>, Extension(db): Extension<Database>
) -> Result<impl IntoResponse, AppError> {

    index::<Country>(db, pagination).await
}

async fn states_index(pagination: Option<Query<Pagination>>, Extension(db): Extension<Database>
) -> Result<impl IntoResponse, AppError> {

    index::<State>(db, pagination).await
}

async fn cities_index(
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>
) -> Result<impl IntoResponse, AppError>
{
    index::<City>(db, pagination).await
}

async fn world_regions_index(
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>
) -> Result<impl IntoResponse, AppError>
{
    index::<WorldRegion>(db, pagination).await
}

async fn world_subregions_index(
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>
) -> Result<impl IntoResponse, AppError>
{
    index::<WorldSubregion>(db, pagination).await
}

async fn currencies_index(
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>
) -> Result<impl IntoResponse, AppError>
{
    index::<Currency>(db, pagination).await
}



//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><====================  OBJECT HANDLERS  =======================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

async fn country_data(Path(key): Path<String>, Extension(db): Extension<Database>) -> Result<impl IntoResponse, AppError> {
    let conn = db.connection()?;
    let country = Country::get(&conn, &key)?;
    let states = State::from_country_count(&conn, &key)?;
    let cities = City::from_country_count(&conn, &key)?;

    let mut headers = HeaderMap::with_capacity(2);
    headers.insert("States-Count", states.into());
    headers.insert("Cities-Count", cities.into());

    Ok( (headers, Json(country)) )
}

async fn state_data(Path(key): Path<String>, Extension(db): Extension<Database>) -> Result<impl IntoResponse, AppError> {
    let conn = db.connection()?;
    let state = State::get(&conn, &key)?;
    let cities = City::from_state_count(&conn, &key)?;

    let mut headers = HeaderMap::with_capacity(1);
    headers.insert("Cities-Count", cities.into());

    Ok( (headers, Json(state)) )
}

async fn city_data(Path(key): Path<String>, Extension(db): Extension<Database>) -> Result<impl IntoResponse, AppError> {
    Ok( Json(City::get(&*db.connection()?, &key)?) )
}

async fn region_data(Path(key): Path<String>, Extension(db): Extension<Database>) -> Result<impl IntoResponse, AppError> {
    let conn = db.connection()?;
    let region = WorldRegion::get(&conn, &key)?;
    let countries = Country::from_region_count(&conn, &key)?;
    let subregions = WorldSubregion::from_region_count(&conn, &key)?;

    let mut headers = HeaderMap::with_capacity(2);
    headers.insert("Countries-Count", countries.into());
    headers.insert("Subregions-Count", subregions.into());

    Ok( (headers, Json(region)) )
}

async fn subregion_data(Path(key): Path<String>, Extension(db): Extension<Database>) -> Result<impl IntoResponse, AppError> {
    let conn = db.connection()?;
    let subregion = WorldSubregion::get(&conn, &key)?;
    let countries = Country::from_subregion_count(&conn, &key)?;

    let mut headers = HeaderMap::with_capacity(1);
    headers.insert("Countries-Count", countries.into());

    Ok( (headers, Json(subregion)) )
}

async fn currency_data(Path(key): Path<String>, Extension(db): Extension<Database>) -> Result<impl IntoResponse, AppError> {
    let conn = db.connection()?;
    let currency = Currency::get(&conn, &key)?;
    let countries = Country::from_currency_count(&conn, &key)?;

    let mut headers = HeaderMap::with_capacity(1);
    headers.insert("Countries-Count", countries.into());

    Ok( (headers, Json(currency)) )
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><===================  FILTERED HANDLERS  ======================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

async fn countries_from_region(
    Path(key): Path<String>,
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>)
-> Result<impl IntoResponse, AppError>
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = Country::from_region(&*db.connection()?, &key, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

async fn countries_from_subregion(
    Path(key): Path<String>,
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>)
-> Result<impl IntoResponse, AppError>
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = Country::from_subregion(&*db.connection()?, &key, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

async fn countries_from_currency(
    Path(key): Path<String>,
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>)
-> Result<impl IntoResponse, AppError>
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = Country::from_currency(&*db.connection()?, &key, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

async fn states_from_country(
    Path(key): Path<String>,
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>)
-> Result<impl IntoResponse, AppError>
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = State::from_country(&*db.connection()?, &key, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

async fn cities_from_country(
    Path(key): Path<String>,
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>)
-> Result<impl IntoResponse, AppError>
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = City::from_country(&*db.connection()?, &key, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

async fn cities_from_state(
    Path(key): Path<String>,
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>)
-> Result<impl IntoResponse, AppError>
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = City::from_state(&*db.connection()?, &key, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

async fn subregions_from_region(
    Path(key): Path<String>,
    pagination: Option<Query<Pagination>>,
    Extension(db): Extension<Database>)
-> Result<impl IntoResponse, AppError>
{
    let Query(pagination) = pagination.unwrap_or_default();
    let (limit, offset) = pagination.to_limit_offset();

    let (total_count, objects) = WorldSubregion::from_region(&*db.connection()?, &key, limit, offset)?;

    Ok(
        (
            pagination_headers(pagination, objects.len(), total_count),
            Json(objects)
        )
    )
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=========================  ERRORS  ===========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// Utility function for mapping any error into a `500 Internal Server Error`
// response.
#[allow(dead_code)]
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=======================  DATABASE  ===========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

#[derive(Clone)]
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new(path: &str) -> Result<Extension<Self>> {
        let manager = SqliteConnectionManager::file(path)
            .with_init(|conn| {
                conn.pragma_update(None, "synchronous", "NORMAL")?;
                conn.pragma_update(None, "foreign_keys", "ON")?;
                Ok(())
            });
        let pool = Pool::new(manager)?;
        Ok(Extension(Self { pool }))
    }

    pub fn connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }
}

pub fn init_db(path: PathBuf) -> Result<Extension<Database>> {
    let db = Database::new(path.to_str().context("invalid unicode on path")?)?;
    let mut conn = db.connection()?;

    conn.pragma_update(None, "journal_mode", "WAL")?;
    // Update the database schema, atomically
    MIGRATIONS.to_latest(&mut conn)?;

    Ok(db)
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=======================  SHUTDOWN  ===========================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
