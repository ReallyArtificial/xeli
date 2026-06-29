pub fn build_prompt(query: &str, schema: &[String], sample_values: &[String]) -> String {
    let schema_str = schema.join("\n  - ");
    let samples_str = sample_values.join("\n  - ");

    format!(
        r#"You are a SQL query generator for DuckDB. Convert the user's natural language question into a valid DuckDB SQL query.

Rules:
- Output ONLY the SQL query. No markdown, no explanation, no code fences.
- The table is named "data".
- Use DuckDB SQL dialect (supports ILIKE, regexp_matches, LIST, STRUCT, etc.).
- Always quote column names with double quotes if they contain spaces or special characters.
- For string matching, prefer ILIKE for case-insensitive matching.
- If the user asks for "top N", use LIMIT N with appropriate ORDER BY.
- If the user asks to "group by" or "aggregate", use GROUP BY with appropriate aggregate functions.
- Return all columns with SELECT * unless the user specifies particular columns.

Table schema:
  - {schema_str}

Sample values:
  - {samples_str}

User question: {query}

SQL:"#
    )
}

/// Prompt the model to design a tracker schema from a natural-language brief,
/// returning JSON that `TableSchema::from_ai_json` parses.
pub fn build_schema_prompt(description: &str) -> String {
    format!(
        r#"Design a spreadsheet/tracker schema from the description. Output ONLY a JSON object — no markdown, no prose.

Shape:
{{"columns":[
  {{"name":"title","type":"text"}},
  {{"name":"status","type":"select","values":["Open","In Progress","Done"],"default":"Open"}},
  {{"name":"owner","type":"person"}},
  {{"name":"due","type":"date"}}
]}}

Rules:
- Allowed "type": text, number, currency, date, bool, person, link, select.
- Use "select" for any column with a small fixed choice set (status, priority, stage, category). Always give such columns a "values" array (3-6 options), and a sensible "default".
- Use short lower_snake_case names. Put a title/name column first.
- 4 to 8 columns total.

Description: {description}

JSON:"#
    )
}
