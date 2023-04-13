
pub fn format_query_params_string(query_start: &str, params_count: usize) -> string_builder::Builder {
    let mut string_builder = string_builder::Builder::new(query_start.len() + params_count * 10);
    string_builder.append(query_start);
    string_builder.append(" (");

    for index in 0..params_count {
        string_builder.append(format!("${}", index + 1));
        if index < (params_count - 1) {
            string_builder.append(", ");
        }
    }

    string_builder.append(")");

    return string_builder
}