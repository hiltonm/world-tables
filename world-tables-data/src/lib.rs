
use rusqlite_migration::{M, Migrations};

lazy_static::lazy_static! {
    pub static ref MIGRATIONS: Migrations<'static> =
        Migrations::new(vec![
            M::up(include_str!("../data/world.sql")),
        ]);
}

// Test that migrations are working
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_test() {
        assert!(MIGRATIONS.validate().is_ok());
    }
}
