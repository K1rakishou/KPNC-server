use anyhow::anyhow;
use tokio_postgres::types::ToSql;

pub fn format_query_params<'a, T : ToSql + Sync>(
    query: &str,
    key: &str,
    params: &'a Vec<T>
) -> anyhow::Result<(String, Vec<&'a (dyn ToSql + Sync)>)> {
    return format_query_params_with_start_index(query, key, 0, params);
}

pub fn format_query_params_with_start_index<'a, T : ToSql + Sync>(
    query: &str,
    key: &str,
    start_index: usize,
    params: &'a Vec<T>
) -> anyhow::Result<(String, Vec<&'a (dyn ToSql + Sync)>)> {
    if params.is_empty() {
        return Err(anyhow!("params are empty!"))
    }

    let index_of_key = query.find(key);
    if index_of_key.is_none() {
        panic!("\'{}\' was not found in query", key);
    }

    let params_count = params.len();
    let index_of_key = index_of_key.unwrap();

    let query_start = &query[..index_of_key];
    let query_end = &query[(index_of_key + key.len())..];
    let total_length = query_start.len() + query_end.len() + (params_count * 4);

    let mut string_builder = string_builder::Builder::new(total_length);
    string_builder.append(query_start);

    let mut index = start_index + 1;

    for _ in 0..params_count {
        string_builder.append(format!("${}", index));
        if index < (params_count + start_index) {
            string_builder.append(", ");
        }

        index += 1;
    }

    string_builder.append(query_end);

    let db_params = params[..]
        .iter()
        .map(|param| param as &(dyn ToSql + Sync))
        .collect::<Vec<&(dyn ToSql + Sync)>>();

    return Ok((string_builder.string()?, db_params));
}

#[test]
fn test_format_query_params_string() {
    let query = "SELECT * FROM test WHERE test.id IN ({QUERY_PARAMS})";
    let params = vec![1, 2, 3, 4, 5];
    let (query, db_params) = format_query_params(
        query,
        "{QUERY_PARAMS}",
        &params
    ).unwrap();

    assert_eq!("SELECT * FROM test WHERE test.id IN ($1, $2, $3, $4, $5)", query);
    assert_eq!(5, db_params.len());
}

#[test]
fn test_format_query_params_string_with_bug() {
    let query = r#"
        SELECT
            post_replies.id
        FROM post_replies
        INNER JOIN accounts account on account.id = post_replies.owner_account_id
        WHERE
            account.account_id = $1
        AND
            post_replies.id IN ({QUERY_PARAMS})
    "#;

    let params = vec![1, 3, 2];
    let (query, db_params) = format_query_params_with_start_index(
        query,
        "{QUERY_PARAMS}",
        1,
        &params
    ).unwrap();

    let expected = r#"
        SELECT
            post_replies.id
        FROM post_replies
        INNER JOIN accounts account on account.id = post_replies.owner_account_id
        WHERE
            account.account_id = $1
        AND
            post_replies.id IN ($2, $3, $4)
    "#;

    assert_eq!(expected, query);
    assert_eq!(3, db_params.len());
}