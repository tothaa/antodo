//! Small, generic SQL primitives. Domain operations compose these against a connection.
use rusqlite::{Connection, OptionalExtension, Params, Row, ToSql, params};

use super::Result;

pub fn execute<P: Params>(connection: &Connection, sql: &str, params: P) -> Result<usize> {
    Ok(connection.execute(sql, params)?)
}

/// Replace an ordered collection inside the caller's transaction. Statements
/// bind the owner, value and zero-based position without interpolating values.
pub(crate) fn replace_ordered<T: ToSql>(
    connection: &Connection,
    delete: &str,
    insert: &str,
    owner: &str,
    values: impl IntoIterator<Item = T>,
) -> Result<()> {
    execute(connection, delete, [owner])?;
    for (position, value) in values.into_iter().enumerate() {
        execute(connection, insert, params![owner, value, position as i64])?;
    }
    Ok(())
}

pub fn query<T, P, F>(connection: &Connection, sql: &str, params: P, map: F) -> Result<Vec<T>>
where
    P: Params,
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let mut statement = connection.prepare(sql)?;
    let values = statement.query_map(params, map)?;
    Ok(values.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn query_optional<T, P, F>(
    connection: &Connection,
    sql: &str,
    params: P,
    map: F,
) -> Result<Option<T>>
where
    P: Params,
    F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
{
    Ok(connection.query_row(sql, params, map).optional()?)
}
