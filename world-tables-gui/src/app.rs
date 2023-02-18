
use anyhow::{Context, Result};
use egui_extras::{Size, StripBuilder};
use egui_extras::{Column, TableBuilder};
use enum_map::{enum_map, EnumMap};
use lazy_static::lazy_static;
use log::debug;
use reqwest::blocking::Client;
use std::{
    cell::RefCell,
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
    thread,
};

use world_tables_base::{
    Tag, Tagged, Keyed, Label, Country, State, City,
    WorldRegion, WorldSubregion, Currency, UrlBuilder, Metadata
};

use crate::types::*;

const RETRY_DELAY: f64 = 10.0;
const PAGE_LIMIT: usize = 100;
const NONE: &str = "None";

lazy_static! {
    static ref LAYOUT_LABEL: egui::Layout = egui::Layout::right_to_left(egui::Align::Center);
    static ref LAYOUT_VALUE: egui::Layout = egui::Layout::left_to_right(egui::Align::Center).with_main_justify(true);
    static ref LAYOUT_BUTTON: egui::Layout = egui::Layout::centered_and_justified(egui::Direction::TopDown);
}

//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><=========================  APP  ==============================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

type ResponseChannels = (Sender<Result<DataResponse>>, Receiver<Result<DataResponse>>);

pub struct App {
    client: Client,
    url: UrlBuilder,

    metadata: ServerData<Metadata>,
    channels: EnumMap<DataKind, ResponseChannels>,
    main_show: EnumMap<MainList, bool>,

    countries: Option<TableData<Country>>,
    states: Option<TableData<State>>,
    cities: Option<TableData<City>>,
    regions: Option<TableData<WorldRegion>>,
    subregions: Option<TableData<WorldSubregion>>,
    currencies: Option<TableData<Currency>>,

    country_windows: HashMap<String, ObjectData<Country>>,
    state_windows: HashMap<String, ObjectData<State>>,
    city_windows: HashMap<String, ObjectData<City>>,
    region_windows: HashMap<String, ObjectData<WorldRegion>>,
    subregion_windows: HashMap<String, ObjectData<WorldSubregion>>,
    currency_windows: HashMap<String, ObjectData<Currency>>,

    countries_by_region_windows: RefCell<HashMap<String, FilteredTableData<Country>>>,
    countries_by_subregion_windows: RefCell<HashMap<String, FilteredTableData<Country>>>,
    countries_by_currency_windows: RefCell<HashMap<String, FilteredTableData<Country>>>,
    states_by_country_windows: RefCell<HashMap<String, FilteredTableData<State>>>,
    cities_by_country_windows: RefCell<HashMap<String, FilteredTableData<City>>>,
    cities_by_state_windows: RefCell<HashMap<String, FilteredTableData<City>>>,
    subregions_by_region_windows: RefCell<HashMap<String, FilteredTableData<WorldSubregion>>>,

    errors: Vec<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap(),
            url: UrlBuilder::new(),
            metadata: ServerData::Empty,

            channels: enum_map! {
                _ => channel(),
            },

            main_show: enum_map! {
                _ => false,
            },

            countries: None,
            states: None,
            cities: None,
            regions: None,
            subregions: None,
            currencies: None,

            country_windows: HashMap::new(),
            state_windows: HashMap::new(),
            city_windows: HashMap::new(),
            region_windows: HashMap::new(),
            subregion_windows: HashMap::new(),
            currency_windows: HashMap::new(),

            countries_by_region_windows: RefCell::new(HashMap::new()),
            countries_by_subregion_windows: RefCell::new(HashMap::new()),
            countries_by_currency_windows: RefCell::new(HashMap::new()),
            states_by_country_windows: RefCell::new(HashMap::new()),
            cities_by_country_windows: RefCell::new(HashMap::new()),
            cities_by_state_windows: RefCell::new(HashMap::new()),
            subregions_by_region_windows: RefCell::new(HashMap::new()),

            errors: Vec::new(),
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, addr: SocketAddr) -> Self {
        use catppuccin_egui::FRAPPE as THEME;
        catppuccin_egui::set_theme(&cc.egui_ctx, THEME);

        let mut style = (*cc.egui_ctx.style()).clone();

        style.spacing.window_margin = egui::style::Margin {
            left: 15.0,
            right: 15.0,
            top: 10.0,
            bottom: 10.0,
        };

        style.spacing.button_padding = egui::vec2(8.0, 1.0);
        style.spacing.icon_spacing = 3.0;
        style.spacing.indent_ends_with_horizontal_line = true;
        style.spacing.item_spacing = egui::vec2(4.0, 5.0);

        style.visuals = egui::style::Visuals {
            dark_mode: true,
            window_rounding: egui::Rounding::same(2.5),
            window_stroke: egui::Stroke::new(0.1, THEME.blue),
            window_shadow: epaint::Shadow { extrusion: 5.0, color: THEME.blue },
            popup_shadow: epaint::Shadow { extrusion: 5.0, color: THEME.blue },
            collapsing_header_frame: true,
            widgets: egui::style::Widgets {
                noninteractive: egui::style::WidgetVisuals {
                    bg_stroke: egui::Stroke {
                        width: 1.0,
                        ..style.visuals.widgets.noninteractive.bg_stroke
                    },
                    rounding: egui::Rounding::same(2.5),
                    fg_stroke: egui::Stroke {
                        width: 1.0,
                        ..style.visuals.widgets.noninteractive.fg_stroke
                    },
                    expansion: 0.0,
                    ..style.visuals.widgets.noninteractive
                },
                inactive: egui::style::WidgetVisuals {
                    weak_bg_fill: THEME.surface1, // darker than default
                    ..style.visuals.widgets.inactive
                },
                hovered: egui::style::WidgetVisuals {
                    weak_bg_fill: THEME.surface2, // fix to remove
                    ..style.visuals.widgets.hovered
                },
                ..style.visuals.widgets
            },
            ..style.visuals
        };

        cc.egui_ctx.set_style(style);

        Self {
            url: UrlBuilder::with_addr(addr).unwrap(),
            ..Default::default()
        }
    }

    fn request(&self, url: &UrlBuilder, data_kind: DataKind, ctx: Option<&egui::Context>) {
        let tx = &self.channels[data_kind].0;
        App::send_request(&self.client, url, data_kind, tx, ctx);
    }

    fn send_request(client: &Client, url: &UrlBuilder, data_kind: DataKind, tx: &Sender<Result<DataResponse>>, ctx: Option<&egui::Context>) {
        let tx = tx.clone();
        let ctx = ctx.cloned();
        let client = client.clone();
        let url = url.clone();

        let get_result = move || -> Result<DataResponse> {
            debug!("{}", url.as_str());

            let response = client
                .get(url.as_str())
                .send()
                .context("Failed fetching countries from server")?;

            let pagination = match data_kind {
                DataKind::Metadata | DataKind::Country | DataKind::State |
                DataKind::City | DataKind::Region | DataKind::Subregion | DataKind::Currency => None,
                _ => Some(Pagination::with_headers(response.headers())?),
            };

            let counts = match data_kind {
                DataKind::Country => Some(Counts::with_country_headers(response.headers())?),
                DataKind::State => Some(Counts::with_state_headers(response.headers())?),
                DataKind::Region => Some(Counts::with_region_headers(response.headers())?),
                DataKind::Subregion => Some(Counts::with_subregion_headers(response.headers())?),
                DataKind::Currency => Some(Counts::with_currency_headers(response.headers())?),
                _ => None,
            };

            Ok(DataResponse {
                response,
                page_text: pagination
                    .map(|pagination| pagination.page.to_string())
                    .unwrap_or("1".to_string()),
                pagination,
                counts,
            })
        };

        thread::spawn(move || {
            let result = get_result();
            tx.send(result).unwrap();
            if let Some(ctx) = ctx { ctx.request_repaint() }
        });
    }

    fn recv_response(&mut self) {
        for (data_kind, (_, rx)) in &self.channels {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Err(e) => self.errors.push(format!("{e:#}")),
                    Ok(data_response) => {
                        match data_kind {
                            DataKind::Metadata => unreachable!(),
                            DataKind::Countries => self.countries = data_response.into(),
                            DataKind::States => self.states = data_response.into(),
                            DataKind::Cities => self.cities = data_response.into(),
                            DataKind::Regions => self.regions = data_response.into(),
                            DataKind::Subregions => self.subregions = data_response.into(),
                            DataKind::Currencies => self.currencies = data_response.into(),
                            DataKind::Country => {
                                let counts = data_response.counts;
                                let opt_country: Option<Country> = data_response.into();
                                if let Some(country) = &opt_country {
                                    let key = country.iso2.to_string();
                                    if let Some(object_data) = self.country_windows.get_mut(&key) {
                                        object_data.data = opt_country;
                                        object_data.counts = counts;
                                    }
                                }
                            },
                            DataKind::State => {
                                let counts = data_response.counts;
                                let opt_state: Option<State> = data_response.into();
                                if let Some(state) = &opt_state {
                                    let key = state.id.to_string();
                                    if let Some(object_data) = self.state_windows.get_mut(&key) {
                                        object_data.data = opt_state;
                                        object_data.counts = counts;
                                    }
                                }
                            },
                            DataKind::City => {
                                let counts = data_response.counts;
                                let opt_city: Option<City> = data_response.into();
                                if let Some(city) = &opt_city {
                                    let key = city.id.to_string();
                                    if let Some(object_data) = self.city_windows.get_mut(&key) {
                                        object_data.data = opt_city;
                                        object_data.counts = counts;
                                    }
                                }
                            },
                            DataKind::Region => {
                                let counts = data_response.counts;
                                let opt_region: Option<WorldRegion> = data_response.into();
                                if let Some(region) = &opt_region {
                                    let key = region.id.to_string();
                                    if let Some(object_data) = self.region_windows.get_mut(&key) {
                                        object_data.data = opt_region;
                                        object_data.counts = counts;
                                    }
                                }
                            },
                            DataKind::Subregion => {
                                let counts = data_response.counts;
                                let opt_subregion: Option<WorldSubregion> = data_response.into();
                                if let Some(subregion) = &opt_subregion {
                                    let key = subregion.id.to_string();
                                    if let Some(object_data) = self.subregion_windows.get_mut(&key) {
                                        object_data.data = opt_subregion;
                                        object_data.counts = counts;
                                    }
                                }
                            },
                            DataKind::Currency => {
                                let counts = data_response.counts;
                                let opt_currency: Option<Currency> = data_response.into();
                                if let Some(currency) = &opt_currency {
                                    let key = currency.iso.to_string();
                                    if let Some(object_data) = self.currency_windows.get_mut(&key) {
                                        object_data.data = opt_currency;
                                        object_data.counts = counts;
                                    }
                                }
                            },
                            DataKind::CountriesByRegion => {
                                let objects: Option<TableData<Country>> = data_response.into();
                                let key: String = {
                                    if let Some(table_data) = &objects {
                                        // should not panic here if the countries button is properly disabled
                                        table_data.data[0].region.key().unwrap().to_string()
                                    } else {
                                        panic!("Region id not found on list of countries from the API");
                                    }
                                };

                                if let Some(filtered_table_data) = self.countries_by_region_windows.borrow_mut().get_mut(&key) {
                                    filtered_table_data.data = objects;
                                }
                            },
                            DataKind::CountriesBySubregion => {
                                let objects: Option<TableData<Country>> = data_response.into();
                                let key: String = {
                                    if let Some(table_data) = &objects {
                                        // should not panic here if the countries button is properly disabled
                                        table_data.data[0].subregion.key().unwrap().to_string()
                                    } else {
                                        panic!("Subregion id not found on list of countries from the API");
                                    }
                                };

                                if let Some(filtered_table_data) = self.countries_by_subregion_windows.borrow_mut().get_mut(&key) {
                                    filtered_table_data.data = objects;
                                }
                            },
                            DataKind::CountriesByCurrency => {
                                let objects: Option<TableData<Country>> = data_response.into();
                                let key: String = {
                                    if let Some(table_data) = &objects {
                                        // should not panic here if the countries button is properly disabled
                                        table_data.data[0].currency.key().unwrap().to_string()
                                    } else {
                                        panic!("Currency id not found on list of countries from the API");
                                    }
                                };

                                if let Some(filtered_table_data) = self.countries_by_currency_windows.borrow_mut().get_mut(&key) {
                                    filtered_table_data.data = objects;
                                }
                            },
                            DataKind::StatesByCountry => {
                                let objects: Option<TableData<State>> = data_response.into();
                                let key: String = {
                                    if let Some(table_data) = &objects {
                                        // should not panic here if the states button is properly disabled
                                        table_data.data[0].country.key().unwrap().to_string()
                                    } else {
                                        panic!("Country id not found on list of states from the API");
                                    }
                                };

                                if let Some(filtered_table_data) = self.states_by_country_windows.borrow_mut().get_mut(&key) {
                                    filtered_table_data.data = objects;
                                }
                            },
                            DataKind::CitiesByCountry => {
                                let objects: Option<TableData<City>> = data_response.into();
                                let key: String = {
                                    if let Some(table_data) = &objects {
                                        // should not panic here if the cities button is properly disabled
                                        table_data.data[0].country.key().unwrap().to_string()
                                    } else {
                                        panic!("Country id not found on list of states from the API");
                                    }
                                };

                                if let Some(filtered_table_data) = self.cities_by_country_windows.borrow_mut().get_mut(&key) {
                                    filtered_table_data.data = objects;
                                }
                            },
                            DataKind::CitiesByState => {
                                let objects: Option<TableData<City>> = data_response.into();
                                let key: String = {
                                    if let Some(table_data) = &objects {
                                        // should not panic here if the cities button is properly disabled
                                        table_data.data[0].state.key().unwrap().to_string()
                                    } else {
                                        panic!("State id not found on list of states from the API");
                                    }
                                };

                                if let Some(filtered_table_data) = self.cities_by_state_windows.borrow_mut().get_mut(&key) {
                                    filtered_table_data.data = objects;
                                }
                            },
                            DataKind::SubregionsByRegion => {
                                let objects: Option<TableData<WorldSubregion>> = data_response.into();
                                let key: String = {
                                    if let Some(table_data) = &objects {
                                        // should not panic here if the cities button is properly disabled
                                        table_data.data[0].region.key().unwrap().to_string()
                                    } else {
                                        panic!("Region id not found on list of states from the API");
                                    }
                                };

                                if let Some(filtered_table_data) = self.subregions_by_region_windows.borrow_mut().get_mut(&key) {
                                    filtered_table_data.data = objects;
                                }
                            },
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn window_table<F>(
        &self,
        ctx: &egui::Context,
        show: &mut bool,
        url: &UrlBuilder,
        data_kind: DataKind,
        list_data: MainListData,
        page_text: Option<String>,
        add_row_content: F
    ) -> Option<String>
    where
        F: FnMut(usize, egui_extras::TableRow<'_, '_>),
    {
        let mut result = None;
        let column_headers = list_data.column_headers();
        let (title, pagination) = list_data.data();

        egui::Window::new(title)
            .open(show)
            .default_size(egui::vec2(column_headers.len() as f32 * 146.0, 300.0))
            .resizable(true)
            .show(ctx, |ui| {
                // remove button frame for table entries
                ui.visuals_mut().button_frame = false;
                ui.add_space(10.0);

                if let Some(pagination) = pagination {
                    StripBuilder::new(ui)
                        .size(Size::remainder())
                        .size(Size::initial(40.0))
                        .vertical(|mut strip| {
                            strip.cell(|ui| {
                                App::data_table(ui, pagination.count, column_headers, add_row_content);
                            });

                            let metadata = self.metadata.unwrap_ref();

                            let count_max = match list_data {
                                MainListData::Countries(..) => metadata.countries,
                                MainListData::States(..) => metadata.states,
                                MainListData::Cities(..) => metadata.cities,
                                MainListData::Regions(..) => metadata.regions,
                                MainListData::Subregions(..) => metadata.subregions,
                                MainListData::Currencies(..) =>  metadata.currencies,
                            };
                            result = self.pagination_strip(ctx, &mut strip, url, data_kind, pagination, page_text.unwrap(), count_max);
                        });
                } else {
                    spinner(ui);
                }
            });

        result
    }

    fn data_table<F>(ui: &mut egui::Ui, rows_count: usize, headers: &[&str], mut add_row_content: F)
    where
        F: FnMut(usize, egui_extras::TableRow<'_, '_>),
    {
        ui.group(|ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                TableBuilder::new(ui)
                    .striped(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .min_scrolled_height(0.0)
                    .resizable(true)
                    .columns(Column::initial(130.0).clip(true), headers.len())
                    .header(20.0, |mut header| {
                        for title in headers {
                            header.col(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.strong(*title);
                                });
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(20.0, rows_count, |index, row| {
                            add_row_content(index, row);
                        });
                    });
            });
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn pagination_strip(
        &self,
        ctx: &egui::Context,
        strip: &mut egui_extras::Strip,
        url: &UrlBuilder,
        data_kind: DataKind,
        pagination: Pagination,
        mut page_text: String,
        count_max: usize,
    ) -> Option<String>
    {
        let mut result = None;

        strip.strip(|builder| {
            builder
                .size(Size::initial(100.0).at_least(100.0))
                .size(Size::remainder())
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        ui.add_space(6.5);
                        ui.vertical(|ui| {
                            ui.small(format!("Page {} of {}", pagination.page, pagination.total_pages));
                            ui.small(format!("{} of {}", pagination.total_count, count_max));
                        });
                    });

                    strip.cell(|ui| {
                        if pagination.total_pages != 1 {
                            ui.visuals_mut().button_frame = true;
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let page = pagination.page;

                                if ui.add_enabled(page < pagination.total_pages, egui::Button::new("Next")).clicked() {
                                    let url = url.clone().with_pagination(page + 1, PAGE_LIMIT);
                                    self.request(&url, data_kind, Some(ctx));
                                }

                                let page_response = ui.add(egui::TextEdit::singleline(&mut page_text).desired_width(25.0));

                                if page_response.changed() {
                                    let page_num: usize = page_text.parse().unwrap_or_default();

                                    if page_num > 0 {
                                        page_text = page_num.min(pagination.total_pages).to_string();
                                    } else {
                                        page_text = Default::default();
                                    }
                                }

                                if page_response.lost_focus() {
                                    let page_num: usize = page_text.parse().unwrap_or_default();

                                    if page_num > 0 {
                                        let url = url.clone().with_pagination(page_num.min(pagination.total_pages), PAGE_LIMIT);
                                        self.request(&url, data_kind, Some(ctx));
                                    }
                                }

                                if ui.add_enabled(page > 1, egui::Button::new("Back")).clicked() {
                                    let url = url.clone().with_pagination(page - 1, PAGE_LIMIT);
                                    self.request(&url, data_kind, Some(ctx));
                                }

                                result = Some(page_text);
                            });
                        }
                    });
                });
        });

        result
    }

    fn handle_selection<T>(
        ctx: &egui::Context,
        client: &Client,
        url: &UrlBuilder,
        channels: &EnumMap<DataKind, ResponseChannels>,
        data_kind: DataKind,
        selection: Option<Tag>,
        windows_map: &mut HashMap<String, ObjectData<T>>)
    {
        if let Some(Tag { key, label }) = selection {
            let skey = key.clone();
            if App::new_window(key, label, windows_map) {
                let tx = &channels[data_kind].0;
                App::send_request(client, &App::object_url(url, data_kind, &skey).unwrap(), data_kind, tx, Some(ctx));
            }
        }
    }

    fn object_url(url: &UrlBuilder, data_kind: DataKind, key: &str) -> Option<UrlBuilder> {
        match data_kind {
            DataKind::Country => Some(url.for_country(key)),
            DataKind::State => Some(url.for_state(key)),
            DataKind::City => Some(url.for_city(key)),
            DataKind::Region => Some(url.for_world_region(key)),
            DataKind::Subregion => Some(url.for_world_subregion(key)),
            DataKind::Currency => Some(url.for_currency(key)),
            _ => None,
        }
    }

    fn new_window<T>(
        key: String,
        label: String,
        windows_map: &mut HashMap<String, ObjectData<T>>) -> bool
    {
        if let Entry::Vacant(e) = windows_map.entry(key) {
            e.insert(
                ObjectData {
                    title: label,
                    ..Default::default()
                }
            );
            return true;
        }

        false
    }

    fn handle_filtered_selection<T>(
        &self,
        ctx: &egui::Context,
        data_kind: DataKind,
        selection: Option<Tag>,
        windows_map: &mut HashMap<String, FilteredTableData<T>>)
    {
        if let Some(Tag { key, label }) = selection {
            if let Entry::Vacant(e) = windows_map.entry(key.clone()) {
                let (title, url) = match data_kind {
                    DataKind::CountriesByRegion => (
                        "Countries",
                        self.url.for_countries_from_region(&key).with_pagination(1, PAGE_LIMIT),
                    ),
                    DataKind::CountriesBySubregion => (
                        "Countries",
                        self.url.for_countries_from_subregion(&key).with_pagination(1, PAGE_LIMIT),
                    ),
                    DataKind::CountriesByCurrency => (
                        "Countries",
                        self.url.for_countries_from_currency(&key).with_pagination(1, PAGE_LIMIT),
                    ),
                    DataKind::StatesByCountry => (
                        "States",
                        self.url.for_states_from_country(&key).with_pagination(1, PAGE_LIMIT),
                    ),
                    DataKind::CitiesByCountry => (
                        "Cities",
                        self.url.for_cities_from_country(&key).with_pagination(1, PAGE_LIMIT),
                    ),
                    DataKind::CitiesByState => (
                        "Cities",
                        self.url.for_cities_from_state(&key).with_pagination(1, PAGE_LIMIT),
                    ),
                    DataKind::SubregionsByRegion => (
                        "Subregions",
                        self.url.for_subregions_from_region(&key).with_pagination(1, PAGE_LIMIT),
                    ),
                    _ => panic!("Data kind not supported for filtered listing"),
                };

                e.insert(
                    FilteredTableData {
                        data: None,
                        show: true,
                        title: format!("{} from {}", title, &label),
                    }
                );
                self.request(&url, data_kind, Some(ctx));
            }
        }
    }

    fn errors_window(&mut self, ctx: &egui::Context) {
        if !self.errors.is_empty() {
            egui::Window::new("Errors")
                .default_size((200.0, 200.0))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for error in &self.errors {
                            ui.label(error);
                        }
                    });
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("Clear").clicked() {
                            self.errors.clear();
                        }
                    });
                });
        }
    }
}

impl eframe::App for App {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><========================  METADATA  ==========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        if !self.metadata.is_ok() {
            match &self.metadata {
                ServerData::Empty => {
                    self.request(&self.url.for_metadata(), DataKind::Metadata, Some(ctx));
                    self.metadata = ServerData::Loading;
                },
                ServerData::Loading => {
                    if let Ok(result) = self.channels[DataKind::Metadata].1.try_recv() {
                        let handle_error = |e| -> ServerData<Metadata> {
                            debug!("{:?}", e);
                            ServerData::Failed(format!("{e:#}"), ctx.input(|i| i.time))
                        };

                        self.metadata = result
                            .and_then(|data_response| Ok(data_response.response.json()?))
                            .map_or_else(handle_error, ServerData::Ok);
                    }
                },
                ServerData::Failed(message, time) => {
                    if ctx.input(|i| i.time) >= time + RETRY_DELAY {
                        self.metadata = ServerData::Empty;
                    } else if !message.is_empty() {
                        self.errors.push(message.clone());
                        self.metadata = ServerData::Failed(Default::default(), *time);
                    }
                },
                _ => unreachable!(),
            }

            egui::CentralPanel::default()
                .show(ctx, |ui| {
                    egui::warn_if_debug_build(ui);
                    ui.centered_and_justified(|ui| {
                        ui.add(egui::Spinner::new().size(50.0));
                    });
                });

            self.errors_window(ctx);

            return;
        }

        let mut main_show = self.main_show;

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><======================  SIDE PANEL  ==========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        egui::SidePanel::left("side_panel")
            .exact_width(130.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("World Tables");
                    ui.separator();
                });

                StripBuilder::new(ui)
                    .sizes(Size::remainder(), 3)
                    .vertical(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            ui.vertical_centered_justified(|ui| {
                                ui.group(|ui| {
                                    let meta = self.metadata.unwrap_ref();
                                    ui.spacing_mut().item_spacing.y = 4.0;
                                    if ui.toggle_value(&mut main_show[MainList::Countries], format!("Countries ({})", meta.countries)).changed() &&
                                        main_show[MainList::Countries]
                                    {
                                        let url = self.url.for_countries().with_pagination(1, PAGE_LIMIT);
                                        self.request(&url, DataKind::Countries, Some(ctx));
                                    }
                                    if ui.toggle_value(&mut main_show[MainList::States], format!("States ({})", meta.states)).changed() &&
                                        main_show[MainList::States]
                                    {
                                        let url = self.url.for_states().with_pagination(1, PAGE_LIMIT);
                                        self.request(&url, DataKind::States, Some(ctx));
                                    }
                                    if ui.toggle_value(&mut main_show[MainList::Cities], format!("Cities ({})", meta.cities)).changed() &&
                                        main_show[MainList::Cities]
                                    {
                                        let url = self.url.for_cities().with_pagination(1, PAGE_LIMIT);
                                        self.request(&url, DataKind::Cities, Some(ctx));
                                    }
                                    if ui.toggle_value(&mut main_show[MainList::Regions], format!("Regions ({})", meta.regions)).changed() &&
                                        main_show[MainList::Regions]
                                    {
                                        let url = self.url.for_world_regions().with_pagination(1, PAGE_LIMIT);
                                        self.request(&url, DataKind::Regions, Some(ctx));
                                    }
                                    if ui.toggle_value(&mut main_show[MainList::Subregions], format!("Subregions ({})", meta.subregions)).changed() &&
                                        main_show[MainList::Subregions]
                                    {
                                        let url = self.url.for_world_subregions().with_pagination(1, PAGE_LIMIT);
                                        self.request(&url, DataKind::Subregions, Some(ctx));
                                    }
                                    if ui.toggle_value(&mut main_show[MainList::Currencies], format!("Currencies ({})", meta.currencies)).changed() &&
                                        main_show[MainList::Currencies]
                                    {
                                        let url = self.url.for_currencies().with_pagination(1, PAGE_LIMIT);
                                        self.request(&url, DataKind::Currencies, Some(ctx));
                                    }
                                });
                            });
                        });
                        strip.empty();
                    });
            });

        egui::CentralPanel::default()
            .show(ctx, |ui| {
                egui::warn_if_debug_build(ui);
            });

        // needed because toggled_value 'changed' response doesn't trigger on
        // window close button
        if !main_show[MainList::Countries] && self.countries.is_some() {
            self.countries = None
        }
        if !main_show[MainList::States] && self.states.is_some() {
            self.states = None
        }
        if !main_show[MainList::Cities] && self.cities.is_some() {
            self.cities = None
        }
        if !main_show[MainList::Regions] && self.regions.is_some() {
            self.regions = None
        }
        if !main_show[MainList::Subregions] && self.subregions.is_some() {
            self.subregions = None
        }
        if !main_show[MainList::Currencies] && self.currencies.is_some() {
            self.currencies = None
        }

        self.recv_response();

        let mut country_selected: Option<Tag> = None;
        let mut state_selected: Option<Tag> = None;
        let mut city_selected: Option<Tag> = None;
        let mut region_selected: Option<Tag> = None;
        let mut subregion_selected: Option<Tag> = None;
        let mut currency_selected: Option<Tag> = None;

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=======================  COUNTRIES  ==========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        let page_text = self.window_table(
            ctx,
            &mut main_show[MainList::Countries],
            &self.url.for_countries(),
            DataKind::Countries,
            MainListData::Countries("Countries", self.countries.as_ref().map(|d| d.pagination)),
            self.countries.as_ref().map(|d| d.page_text.clone()),
            |index, mut row|
        {
            let country = &self.countries.as_ref().unwrap().data[index];
            col_button(&mut row, country, &mut country_selected);
            col_button(&mut row, &country.region, &mut region_selected);
            col_button(&mut row, &country.subregion, &mut subregion_selected);
        });

        if let Some(page_text) = page_text {
            self.countries.as_mut().unwrap().page_text = page_text;
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=========================  STATES  ===========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        let page_text = self.window_table(
            ctx,
            &mut main_show[MainList::States],
            &self.url.for_states(),
            DataKind::States,
            MainListData::States("States", self.states.as_ref().map(|d| d.pagination)),
            self.states.as_ref().map(|d| d.page_text.clone()),
            |index, mut row|
        {
            let state = &self.states.as_ref().unwrap().data[index];
            col_button(&mut row, state, &mut state_selected);
            col_button(&mut row, &state.country, &mut country_selected);
        });

        if let Some(page_text) = page_text {
            self.states.as_mut().unwrap().page_text = page_text;
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=========================  CITIES  ===========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        let page_text = self.window_table(
            ctx,
            &mut main_show[MainList::Cities],
            &self.url.for_cities(),
            DataKind::Cities,
            MainListData::Cities("Cities", self.cities.as_ref().map(|d| d.pagination)),
            self.cities.as_ref().map(|d| d.page_text.clone()),
            |index, mut row|
        {
            let city = &self.cities.as_ref().unwrap().data[index];
            col_button(&mut row, city, &mut city_selected);
            col_button(&mut row, &city.state, &mut state_selected);
            col_button(&mut row, &city.country, &mut country_selected);
        });

        if let Some(page_text) = page_text {
            self.cities.as_mut().unwrap().page_text = page_text;
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=========================  REGIONS  ==========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        let page_text = self.window_table(
            ctx,
            &mut main_show[MainList::Regions],
            &self.url.for_world_regions(),
            DataKind::Regions,
            MainListData::Regions("Regions", self.regions.as_ref().map(|d| d.pagination)),
            self.regions.as_ref().map(|d| d.page_text.clone()),
            |index, mut row|
        {
            let region = &self.regions.as_ref().unwrap().data[index];
            col_button(&mut row, region, &mut region_selected);
        });

        if let Some(page_text) = page_text {
            self.regions.as_mut().unwrap().page_text = page_text;
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=======================  SUBREGIONS  =========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        let page_text = self.window_table(
            ctx,
            &mut main_show[MainList::Subregions],
            &self.url.for_world_subregions(),
            DataKind::Subregions,
            MainListData::Subregions("Subregions", self.subregions.as_ref().map(|d| d.pagination)),
            self.subregions.as_ref().map(|d| d.page_text.clone()),
            |index, mut row|
        {
            let subregion = &self.subregions.as_ref().unwrap().data[index];
            col_button(&mut row, subregion, &mut subregion_selected);
            col_button(&mut row, &subregion.region, &mut region_selected);
        });

        if let Some(page_text) = page_text {
            self.subregions.as_mut().unwrap().page_text = page_text;
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=======================  CURRENCIES  =========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        let page_text = self.window_table(
            ctx,
            &mut main_show[MainList::Currencies],
            &self.url.for_currencies(),
            DataKind::Currencies,
            MainListData::Currencies("Currencies", self.currencies.as_ref().map(|d| d.pagination)),
            self.currencies.as_ref().map(|d| d.page_text.clone()),
            |index, mut row|
        {
            let currency = &self.currencies.as_ref().unwrap().data[index];
            col_button(&mut row, currency, &mut currency_selected);
            col_label(&mut row, currency.iso.as_deref().unwrap());
            col_label(&mut row, &currency.symbol);
        });

        if let Some(page_text) = page_text {
            self.currencies.as_mut().unwrap().page_text = page_text;
        }

        // persist the state of shows
        self.main_show = main_show;

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><===================  COUNTRY WINDOWS  ========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;
            let mut states_by_country_selected: Option<Tag> = None;
            let mut cities_by_country_selected: Option<Tag> = None;

            for (key, object) in self.country_windows.iter_mut() {
                egui::Window::new(&object.title)
                    .id(format!("country:{}", &object.title).into())
                    .open(&mut object.show)
                    .default_size(egui::vec2(50.0, 50.0))
                    .resizable(false)
                    .show(ctx, |ui| {
                        if let Some(country) = &object.data {
                            ui.group(|ui| {
                                egui::Grid::new(&country.name).striped(true).num_columns(2).show(ui, |ui| {
                                    data_value(ui, "ISO 2:", country.iso2.as_deref());
                                    data_value(ui, "ISO 3:", Some(&country.iso3));
                                    data_value(ui, "Code:", Some(&country.code.to_string()));
                                    data_value(ui, "TLD:", Some(&country.tld));
                                    data_value(ui, "Native:", Some(&country.native));
                                    data_value(ui, "Latitude:", Some(&format!("{:.8}", country.latitude)));
                                    data_value(ui, "Longitude:", Some(&format!("{:.8}", country.longitude)));
                                    data_button(ui, "Capital:", &country.capital, &mut city_selected);
                                    data_button(ui, "Currency:", &country.currency, &mut currency_selected);
                                    data_button(ui, "Region:", &country.region, &mut region_selected);
                                    data_button(ui, "Subregion:", &country.subregion, &mut subregion_selected);
                                });
                            });

                            ui.group(|ui| {
                                if let Some(Counts::Country { states, cities }) = object.counts {
                                    ui.columns(2, |columns| {
                                        filtered_button(&mut columns[0], "States", states, country, &mut states_by_country_selected);
                                        filtered_button(&mut columns[1], "Cities", cities, country, &mut cities_by_country_selected);
                                    });
                                }
                            });
                        } else {
                            spinner(ui);
                        }
                    });

                if !object.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                self.country_windows.remove(&key);
            }

            self.handle_filtered_selection(
                ctx,
                DataKind::StatesByCountry,
                states_by_country_selected,
                &mut *self.states_by_country_windows.borrow_mut()
            );

            self.handle_filtered_selection(
                ctx,
                DataKind::CitiesByCountry,
                cities_by_country_selected,
                &mut *self.cities_by_country_windows.borrow_mut()
            );
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=====================  STATE WINDOWS  ========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;
            let mut cities_by_state_selected: Option<Tag> = None;

            for (key, object) in self.state_windows.iter_mut() {
                egui::Window::new(&object.title)
                    .id(format!("state:{}", &object.title).into())
                    .open(&mut object.show)
                    .default_size(egui::vec2(50.0, 50.0))
                    .resizable(false)
                    .show(ctx, |ui| {
                        if let Some(state) = &object.data {
                            ui.group(|ui| {
                                egui::Grid::new(&state.name).striped(true).num_columns(2).show(ui, |ui| {
                                    data_value(ui, "Code:", Some(&state.code.to_string()));
                                    data_value(ui, "Latitude:", state.latitude.map(|v| format!("{v:.8}")).as_deref());
                                    data_value(ui, "Longitude:", state.longitude.map(|v| format!("{v:.8}")).as_deref());
                                    data_button(ui, "Country:", &state.country, &mut country_selected);
                                });
                            });

                            ui.group(|ui| {
                                if let Some(Counts::State { cities }) = object.counts {
                                    filtered_button(ui, "Cities", cities, state, &mut cities_by_state_selected);
                                }
                            });
                        } else {
                            spinner(ui);
                        }
                    });

                if !object.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                self.state_windows.remove(&key);
            }

            self.handle_filtered_selection(
                ctx,
                DataKind::CitiesByState,
                cities_by_state_selected,
                &mut *self.cities_by_state_windows.borrow_mut()
            );
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><======================  CITY WINDOWS  ========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;

            for (key, object) in self.city_windows.iter_mut() {
                egui::Window::new(&object.title)
                    .id(format!("city:{}", &object.title).into())
                    .open(&mut object.show)
                    .default_size(egui::vec2(50.0, 50.0))
                    .resizable(false)
                    .show(ctx, |ui| {
                        if let Some(city) = &object.data {
                            ui.group(|ui| {
                                egui::Grid::new(&city.name).striped(true).num_columns(2).show(ui, |ui| {
                                    data_value(ui, "Latitude:", city.latitude.map(|v| format!("{v:.8}")).as_deref());
                                    data_value(ui, "Longitude:", city.longitude.map(|v| format!("{v:.8}")).as_deref());
                                    data_button(ui, "State:", &city.state, &mut state_selected);
                                    data_button(ui, "Country:", &city.country, &mut country_selected);
                                });
                            });
                        } else {
                            spinner(ui);
                        }
                    });

                if !object.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                self.city_windows.remove(&key);
            }
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><====================  REGION WINDOWS  ========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;
            let mut countries_by_region_selected: Option<Tag> = None;
            let mut subregions_by_region_selected: Option<Tag> = None;

            for (key, object) in self.region_windows.iter_mut() {
                egui::Window::new(&object.title)
                    .id(format!("region:{}", &object.title).into())
                    .open(&mut object.show)
                    .default_size(egui::vec2(50.0, 50.0))
                    .resizable(false)
                    .show(ctx, |ui| {
                        if let Some(region) = &object.data {
                            ui.group(|ui| {
                                egui::Grid::new(&region.name).striped(true).num_columns(2).show(ui, |ui| {
                                    data_value(ui, "Name:", Some(&region.name));
                                });
                            });

                            ui.group(|ui| {
                                if let Some(Counts::Region { countries, subregions }) = object.counts {
                                    ui.columns(2, |columns| {
                                        filtered_button(&mut columns[0], "Countries", countries, region, &mut countries_by_region_selected);
                                        filtered_button(&mut columns[1], "Subregions", subregions, region, &mut subregions_by_region_selected);
                                    });
                                }
                            });
                        } else {
                            spinner(ui);
                        }
                    });

                if !object.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                self.region_windows.remove(&key);
            }

            self.handle_filtered_selection(
                ctx,
                DataKind::CountriesByRegion,
                countries_by_region_selected,
                &mut *self.countries_by_region_windows.borrow_mut()
            );

            self.handle_filtered_selection(
                ctx,
                DataKind::SubregionsByRegion,
                subregions_by_region_selected,
                &mut *self.subregions_by_region_windows.borrow_mut()
            );
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><===================  SUBREGION WINDOWS  ======================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;
            let mut countries_by_subregion_selected: Option<Tag> = None;

            for (key, object) in self.subregion_windows.iter_mut() {
                egui::Window::new(&object.title)
                    .id(format!("subregion:{}", &object.title).into())
                    .open(&mut object.show)
                    .default_size(egui::vec2(50.0, 50.0))
                    .resizable(false)
                    .show(ctx, |ui| {
                        if let Some(subregion) = &object.data {
                            ui.group(|ui| {
                                egui::Grid::new(&subregion.name).striped(true).num_columns(2).show(ui, |ui| {
                                    data_value(ui, "Name:", Some(&subregion.name));
                                    data_button(ui, "Region:", &subregion.region, &mut region_selected);
                                });
                            });

                            ui.group(|ui| {
                                if let Some(Counts::Subregion { countries }) = object.counts {
                                    filtered_button(ui, "Countries", countries, subregion, &mut countries_by_subregion_selected);
                                }
                            });
                        } else {
                            spinner(ui);
                        }
                    });

                if !object.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                self.subregion_windows.remove(&key);
            }

            self.handle_filtered_selection(
                ctx,
                DataKind::CountriesBySubregion,
                countries_by_subregion_selected,
                &mut *self.countries_by_subregion_windows.borrow_mut()
            );
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><====================  CURRENCY WINDOWS  ======================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;
            let mut countries_by_currency_selected: Option<Tag> = None;

            for (key, object) in self.currency_windows.iter_mut() {
                egui::Window::new(&object.title)
                    .id(format!("currency:{}", &object.title).into())
                    .open(&mut object.show)
                    .default_size(egui::vec2(50.0, 50.0))
                    .resizable(false)
                    .show(ctx, |ui| {
                        if let Some(currency) = &object.data {
                            ui.group(|ui| {
                                egui::Grid::new(&currency.name).striped(true).num_columns(2).show(ui, |ui| {
                                    data_value(ui, "Name:", Some(&currency.name));
                                    data_value(ui, "ISO:", currency.iso.as_deref());
                                    data_value(ui, "Symbol:", Some(&currency.symbol));
                                });
                            });

                            ui.group(|ui| {
                                if let Some(Counts::Currency { countries }) = object.counts {
                                    filtered_button(ui, "Countries", countries, currency, &mut countries_by_currency_selected);
                                }
                            });
                        } else {
                            spinner(ui);
                        }
                    });

                if !object.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                self.currency_windows.remove(&key);
            }

            self.handle_filtered_selection(
                ctx,
                DataKind::CountriesByCurrency,
                countries_by_currency_selected,
                &mut *self.countries_by_currency_windows.borrow_mut()
            );
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><====================  FILTERED COUNTRIES =====================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let windows: [(DataKind, &mut HashMap<String, FilteredTableData<Country>>); 3] = [
                (
                    DataKind::CountriesByRegion,
                    &mut *self.countries_by_region_windows.borrow_mut(),
                ),
                (
                    DataKind::CountriesBySubregion,
                    &mut *self.countries_by_subregion_windows.borrow_mut(),
                ),
                (
                    DataKind::CountriesByCurrency,
                    &mut *self.countries_by_currency_windows.borrow_mut(),
                ),
            ];

            for (data_kind, countries_windows) in windows {
                let mut garbage: Option<String> = None;

                let url_builder: Box<dyn Fn(&str) -> UrlBuilder> = match data_kind {
                    DataKind::CountriesByRegion => Box::new(|key| self.url.for_countries_from_region(key)),
                    DataKind::CountriesBySubregion => Box::new(|key| self.url.for_countries_from_subregion(key)),
                    DataKind::CountriesByCurrency => Box::new(|key| self.url.for_countries_from_currency(key)),
                    _ => unreachable!(),
                };

                for (key, filtered_table_data) in &mut *countries_windows {
                    let page_text = self.window_table(
                        ctx,
                        &mut filtered_table_data.show,
                        &url_builder(key),
                        data_kind,
                        MainListData::Countries(&filtered_table_data.title, filtered_table_data.data.as_ref().map(|d| d.pagination)),
                        filtered_table_data.data.as_ref().map(|d| d.page_text.clone()),
                        |index, mut row| {
                            let country = &filtered_table_data.data.as_ref().unwrap().data[index];
                            col_button(&mut row, country, &mut country_selected);
                            col_button(&mut row, &country.region, &mut region_selected);
                            col_button(&mut row, &country.subregion, &mut subregion_selected);
                        });

                    if let Some(page_text) = page_text {
                        filtered_table_data.data.as_mut().unwrap().page_text = page_text;
                    }

                    if !filtered_table_data.show {
                        garbage = Some(key.clone());
                    }
                }

                if let Some(key) = garbage {
                    countries_windows.remove(&key);
                }
            }
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=====================  FILTERED STATES  ======================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;
            let mut states_windows = self.states_by_country_windows.borrow_mut();

            for (key, filtered_table_data) in &mut *states_windows {
                let page_text = self.window_table(
                    ctx,
                    &mut filtered_table_data.show,
                    &self.url.for_states_from_country(key),
                    DataKind::StatesByCountry,
                    MainListData::States(&filtered_table_data.title, filtered_table_data.data.as_ref().map(|d| d.pagination)),
                    filtered_table_data.data.as_ref().map(|d| d.page_text.clone()),
                    |index, mut row| {
                        let state = &filtered_table_data.data.as_ref().unwrap().data[index];
                        col_button(&mut row, state, &mut state_selected);
                        col_button(&mut row, &state.country, &mut country_selected);
                    });

                if let Some(page_text) = page_text {
                    filtered_table_data.data.as_mut().unwrap().page_text = page_text;
                }

                if !filtered_table_data.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                states_windows.remove(&key);
            }
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><======================  FILTERED CITIES ======================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let windows: [(DataKind, &mut HashMap<String, FilteredTableData<City>>); 2] = [
                (
                    DataKind::CitiesByCountry,
                    &mut *self.cities_by_country_windows.borrow_mut(),
                ),
                (
                    DataKind::CitiesByState,
                    &mut *self.cities_by_state_windows.borrow_mut(),
                ),
            ];

            for (data_kind, cities_windows) in windows {
                let mut garbage: Option<String> = None;

                let url_builder: Box<dyn Fn(&str) -> UrlBuilder> = match data_kind {
                    DataKind::CitiesByCountry => Box::new(|key| self.url.for_cities_from_country(key)),
                    DataKind::CitiesByState => Box::new(|key| self.url.for_cities_from_state(key)),
                    _ => unreachable!(),
                };

                for (key, filtered_table_data) in &mut *cities_windows {
                    let page_text = self.window_table(
                        ctx,
                        &mut filtered_table_data.show,
                        &url_builder(key),
                        data_kind,
                        MainListData::Cities(&filtered_table_data.title, filtered_table_data.data.as_ref().map(|d| d.pagination)),
                        filtered_table_data.data.as_ref().map(|d| d.page_text.clone()),
                        |index, mut row| {
                            let city = &filtered_table_data.data.as_ref().unwrap().data[index];
                            col_button(&mut row, city, &mut city_selected);
                            col_button(&mut row, &city.state, &mut state_selected);
                            col_button(&mut row, &city.country, &mut country_selected);
                        });

                    if let Some(page_text) = page_text {
                        filtered_table_data.data.as_mut().unwrap().page_text = page_text;
                    }

                    if !filtered_table_data.show {
                        garbage = Some(key.clone());
                    }
                }

                if let Some(key) = garbage {
                    cities_windows.remove(&key);
                }
            }
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><==================  FILTERED SUBREGIONS  =====================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        {
            let mut garbage: Option<String> = None;
            let mut subregions_windows = self.subregions_by_region_windows.borrow_mut();

            for (key, filtered_table_data) in &mut *subregions_windows {
                let page_text = self.window_table(
                    ctx,
                    &mut filtered_table_data.show,
                    &self.url.for_subregions_from_region(key),
                    DataKind::SubregionsByRegion,
                    MainListData::Subregions(&filtered_table_data.title, filtered_table_data.data.as_ref().map(|d| d.pagination)),
                    filtered_table_data.data.as_ref().map(|d| d.page_text.clone()),
                    |index, mut row| {
                        let subregion = &filtered_table_data.data.as_ref().unwrap().data[index];
                        col_button(&mut row, subregion, &mut subregion_selected);
                        col_button(&mut row, &subregion.region, &mut region_selected);
                    });

                if let Some(page_text) = page_text {
                    filtered_table_data.data.as_mut().unwrap().page_text = page_text;
                }

                if !filtered_table_data.show {
                    garbage = Some(key.clone());
                }
            }

            if let Some(key) = garbage {
                subregions_windows.remove(&key);
            }
        }

        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
        //<<>><=======================  SELECTION  ==========================><<>>//
        //<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

        App::handle_selection(ctx, &self.client, &self.url, &self.channels, DataKind::Country, country_selected, &mut self.country_windows);
        App::handle_selection(ctx, &self.client, &self.url, &self.channels, DataKind::State, state_selected, &mut self.state_windows);
        App::handle_selection(ctx, &self.client, &self.url, &self.channels, DataKind::City, city_selected, &mut self.city_windows);
        App::handle_selection(ctx, &self.client, &self.url, &self.channels, DataKind::Region, region_selected, &mut self.region_windows);
        App::handle_selection(ctx, &self.client, &self.url, &self.channels, DataKind::Subregion, subregion_selected, &mut self.subregion_windows);
        App::handle_selection(ctx, &self.client, &self.url, &self.channels, DataKind::Currency, currency_selected, &mut self.currency_windows);

        self.errors_window(ctx);
    }
}


//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//
//<<>><======================  GUI COMPONENTS  ======================><<>>//
//<<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>><<>>//

fn data_value(ui: &mut egui::Ui, label: &str, value: Option<&str>) {
    ui.with_layout(*LAYOUT_LABEL, |ui| {
        ui.strong(label);
    });
    ui.with_layout(*LAYOUT_VALUE, |ui| {
        if let Some(value) = value {
            ui.label(value);
        } else {
            ui.weak(NONE);
        }
    });
    ui.end_row();
}

fn data_button<T>(ui: &mut egui::Ui, label: &str, data: &T, selection: &mut Option<Tag>)
where
    T: Tagged + Label<LabelType = String>,
{
    ui.with_layout(*LAYOUT_LABEL, |ui| {
        ui.strong(label);
    });
    ui.with_layout(*LAYOUT_VALUE, |ui| {
        let _ = data.label().map(|value| {
            if value.is_empty() {
                ui.weak(NONE);
            } else if ui.button(value).clicked() {
                *selection = data.tag().ok();
            }
        });
    });
    ui.end_row();
}

fn filtered_button<T>(ui: &mut egui::Ui, label: &str, count: usize, data: &T, selection: &mut Option<Tag>)
where
    T: Tagged,
{
    ui.with_layout(*LAYOUT_BUTTON, |ui| {
        if ui.add_enabled(count > 0, egui::Button::new(format!("{label} ({count})")).wrap(false)).clicked() {
            *selection = data.tag().ok();
        }
    });
}

fn col_label(row: &mut egui_extras::TableRow, label: &str) {
    row.col(|ui| {
        ui.label(label).on_hover_text(label);
    });
}

fn col_button<T>(row: &mut egui_extras::TableRow, data: &T, selection: &mut Option<Tag>)
where
    T: Tagged + Label<LabelType = String>,
{
    row.col(|ui| {
        let _ = data.label().map(|label| {
            if label.is_empty() {
                ui.add_enabled(false, egui::Button::new(NONE));
            } else if ui.button(label).on_hover_text(label).clicked() {
                *selection = data.tag().ok();
            }
        });
    });
}

fn spinner(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.spinner();
    });
}

