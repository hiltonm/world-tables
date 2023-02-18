CREATE TABLE countries (
	iso2				TEXT PRIMARY KEY CHECK(iso2 <> ''),
	iso3				TEXT NOT NULL UNIQUE CHECK(iso3 <> ''),
	name				TEXT NOT NULL UNIQUE CHECK(name <> ''),
	code				INTEGER NOT NULL UNIQUE,
	capital_id			INTEGER,
	capital				TEXT,
	currency_id			TEXT,
	currency			TEXT,
	tld					TEXT NOT NULL,
	native				TEXT NOT NULL,
	world_region_id		INTEGER,
	world_region		TEXT,
	world_subregion_id	INTEGER,
	world_subregion		TEXT,
	latitude			REAL NOT NULL,
	longitude			REAL NOT NULL,
	emoji				TEXT NOT NULL,
	emoji_u				TEXT NOT NULL,
	FOREIGN KEY(capital_id) REFERENCES cities(id),
	FOREIGN KEY(currency_id) REFERENCES currencies(iso),
	FOREIGN KEY(world_region_id) REFERENCES world_regions(id),
	FOREIGN KEY(world_subregion_id) REFERENCES world_subregions(id)
) STRICT;

CREATE TABLE states (
	id			INTEGER PRIMARY KEY,
	name		TEXT NOT NULL CHECK(name <> ''),
	country_id	TEXT NOT NULL,
	country		TEXT NOT NULL CHECK(country <> ''),
	code		TEXT NOT NULL,
	latitude	REAL,
	longitude	REAL,
	FOREIGN KEY(country_id) REFERENCES countries(iso2)
) STRICT;

CREATE TABLE cities (
	id			INTEGER PRIMARY KEY,
	name		TEXT NOT NULL CHECK(name <> ''),
	state_id	INTEGER,
	state		TEXT CHECK(state <> ''),
	country_id	TEXT NOT NULL,
	country		TEXT NOT NULL CHECK(country <> ''),
	latitude	REAL,
	longitude	REAL,
	FOREIGN KEY(state_id) REFERENCES states(id),
	FOREIGN KEY(country_id) REFERENCES countries(iso2)
) STRICT;

CREATE TABLE currencies (
	iso		TEXT PRIMARY KEY CHECK(iso <> ''),
	name	TEXT NOT NULL CHECK(name <> ''),
	symbol	TEXT NOT NULL CHECK(symbol <> '')
) STRICT;

CREATE TABLE world_regions (
	id		INTEGER PRIMARY KEY,
	name	TEXT NOT NULL CHECK(name <> '')
) STRICT;

CREATE TABLE world_subregions (
	id				INTEGER PRIMARY KEY,
	world_region_id	INTEGER NOT NULL,
	name			TEXT NOT NULL CHECK(name <> ''),
	FOREIGN KEY(world_region_id) REFERENCES world_regions(id)
) STRICT;

INSERT INTO world_regions (id, name) VALUES
(1, 'Africa'),
(2, 'Americas'),
(3, 'Asia'),
(4, 'Europe'),
(5, 'Oceania'),
(6, 'Polar');

INSERT INTO world_subregions (id, world_region_id, name) VALUES
(1, 1, 'Eastern Africa'),
(2, 1, 'Middle Africa'),
(3, 1, 'Northern Africa'),
(4, 1, 'Southern Africa'),
(5, 1, 'Western Africa'),
(6, 2, 'Caribbean'),
(7, 2, 'Central America'),
(8, 2, 'Northern America'),
(9, 2, 'South America'),
(10, 3, 'Central Asia'),
(11, 3, 'Eastern Asia'),
(12, 3, 'Southern Asia'),
(13, 3, 'South-Eastern Asia'),
(14, 3, 'Western Asia'),
(15, 4, 'Eastern Europe'),
(16, 4, 'Northern Europe'),
(17, 4, 'Southern Europe'),
(18, 4, 'Western Europe'),
(19, 5, 'Australia and New Zealand'),
(20, 5, 'Melanesia'),
(21, 5, 'Micronesia'),
(22, 5, 'Polynesia');
