use serde::Deserialize;

#[derive(Clone, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
#[serde(field_identifier, rename_all = "lowercase")]
pub enum DatabaseType {
    MySQL,
    MariaDB,
    Postgres,
    Sqlite,
}
