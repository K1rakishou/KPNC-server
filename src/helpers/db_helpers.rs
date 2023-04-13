use tokio_postgres::types::ToSql;

pub fn format_query_params_string(
    query_start: &str,
    query_end: &str,
    params_count: usize
) -> string_builder::Builder {
    let total_length = query_start.len() + query_end.len() + (params_count * 10);

    let mut string_builder = string_builder::Builder::new(total_length);
    string_builder.append(query_start);
    string_builder.append(" (");

    for index in 0..params_count {
        string_builder.append(format!("${}", index + 1));
        if index < (params_count - 1) {
            string_builder.append(", ");
        }
    }

    string_builder.append(")");
    string_builder.append(query_end);

    return string_builder
}

pub fn to_db_params<T : ToSql + Sync>(params: &Vec<T>) -> Vec<&(dyn ToSql + Sync)> {
    return params[..]
        .iter()
        .map(|param| param as &(dyn ToSql + Sync))
        .collect::<Vec<&(dyn ToSql + Sync)>>();
}